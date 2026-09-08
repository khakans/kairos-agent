mod journal;
pub mod models;
pub mod services;
#[cfg(test)]
mod settlement_tests;
use fs2::FileExt;
use kairos_domain::{bps, money, RiskInput, RiskPolicy, TradingMode, USDC};
use kairos_live::{now, vault, LiveExecutor};
use models::*;
use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::{prelude::ToPrimitive, Decimal};
use serde_json::{json, Value};
use services::{AgentReport, Assessment, DecisionAction, Services};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use uuid::Uuid;
use zeroize::Zeroizing;

fn id() -> String {
    Uuid::new_v4().to_string()
}

pub struct Application {
    _runtime_lock: std::fs::File,
    db: Connection,
    directory: PathBuf,
    pub state: RuntimeState,
    live: Option<LiveExecutor>,
    services: Arc<Services>,
    journal_root: PathBuf,
    journal_queue: Vec<Value>,
    journal_error: Option<String>,
    pub kill: Arc<AtomicBool>,
}
impl Application {
    pub fn open(directory: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(directory).map_err(|_| "Cannot create runtime data directory")?;
        let runtime_lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(directory.join("runtime.lock"))
            .map_err(|_| "Cannot open runtime lock")?;
        runtime_lock
            .try_lock_exclusive()
            .map_err(|_| "Another runtime already owns this data directory")?;
        let db =
            Connection::open(directory.join("kairos.sqlite")).map_err(|_| "Cannot open storage")?;
        db.execute_batch(journal::SCHEMA)
            .map_err(|_| "Cannot migrate trade journal")?;
        db.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| "Cannot configure storage")?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS runtime_state (id INTEGER PRIMARY KEY CHECK(id=1), payload TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS audit_events (sequence INTEGER PRIMARY KEY, payload TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS command_receipts (id TEXT PRIMARY KEY, error TEXT);
            CREATE TABLE IF NOT EXISTS dry_run_sessions (id TEXT PRIMARY KEY, payload TEXT NOT NULL);
            PRAGMA user_version=1;").map_err(|_| "Storage migration failed")?;
        let saved: Option<String> = db
            .query_row("SELECT payload FROM runtime_state WHERE id=1", [], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|_| "Cannot read state")?;
        let mut state = match saved {
            Some(saved) => serde_json::from_str::<RuntimeState>(&saved)
                .map_err(|_| "Stored runtime state is invalid; recovery required")?,
            None => initial_state(),
        };
        if state.schema_version != 1 {
            return Err("Unsupported runtime schema".into());
        }
        // Existing replay history remains archived, never relabelled as live market evidence.
        state.markets.clear();
        state.dry_markets.clear();
        if let Some(session) = state.session.as_mut() {
            if session.source != "Jupiter live market" {
                session.status = "Stopped".into();
                for proposal in &mut state.proposals {
                    if proposal.mode == TradingMode::DryRun && proposal.status == "Pending" {
                        proposal.status = "Expired".into();
                    }
                }
            }
        }
        state.agents = initial_agents(state.mode);
        state.live.public_key = vault::public_key(&directory.join("vault.json"));
        state.live.activation_expires_at = 0;
        state.live.unlocked = false;
        state.paused = true;
        if let Some(session) = state.session.as_mut() {
            if session.status == "Running" {
                session.status = "Paused".into();
            }
        }
        let kill = Arc::new(AtomicBool::new(state.killed));
        let mut app = Self {
            _runtime_lock: runtime_lock,
            db,
            directory: directory.into(),
            state,
            live: None,
            services: Arc::new(Services::from_env()?),
            journal_root: directory.join("trade-log"),
            journal_queue: Vec::new(),
            journal_error: None,
            kill,
        };
        journal::recover(&app.db, &app.journal_root)?;
        app.event(
            "runtime.recovered",
            "Storage ready. Entries paused; live authorization must be renewed after restart.",
            "boot",
        );
        app.persist(None)?;
        Ok(app)
    }
    #[cfg(feature = "test-support")]
    pub fn use_test_services(&mut self, base: &str) {
        self.services = Arc::new(Services::for_test(base));
    }
    fn scope(&self) -> String {
        if self.state.mode == TradingMode::DryRun {
            self.state
                .session
                .as_ref()
                .map(|s| s.id.clone())
                .unwrap_or_default()
        } else {
            self.state.live.public_key.clone().unwrap_or_default()
        }
    }
    fn event(&mut self, kind: &str, message: &str, correlation: &str) {
        self.state.sequence += 1;
        self.state.events.push(AuditEvent {
            id: id(),
            sequence: self.state.sequence,
            mode: self.state.mode,
            scope_id: Some(self.scope()).filter(|s| !s.is_empty()),
            timestamp: now(),
            kind: kind.into(),
            message: message.into(),
            correlation_id: correlation.into(),
        });
        if kind.starts_with("execution.")
            || kind.starts_with("position.")
            || kind.starts_with("risk.")
            || kind.starts_with("agent.")
            || kind == "command.rejected"
        {
            let scope = self.scope();
            self.journal_queue.push(json!({
                "schema_version":1, "timezone":"Asia/Jakarta", "market_source":"Jupiter live market",
                "event":self.state.events.last(),
                "markets":self.state.markets, "market_slot":self.state.market_slot,
                "quote":self.state.last_quote, "decision":self.state.last_decision,
                "agents":if kind.starts_with("agent.") { Some(&self.state.agents) } else { None },
                "policy":self.state.policy, "portfolio":if self.state.mode==TradingMode::Live && kind=="execution.reconciled" { None } else { Some(self.portfolio()) },
                "positions":self.state.positions.iter().filter(|p| p.mode==self.state.mode && p.scope_id==scope && p.status=="Open").take(20).collect::<Vec<_>>(),
                "position":self.state.orders.last().filter(|_| matches!(kind,"execution.virtual_filled"|"position.closed"|"execution.reconciled")).and_then(|o|self.state.positions.iter().find(|p| Some(&p.id)==o.position_id.as_ref() && p.mode==self.state.mode && p.scope_id==scope)),
                "order":self.state.orders.last().filter(|o| o.mode==self.state.mode && o.scope_id==scope && matches!(kind,"execution.virtual_filled"|"position.closed"|"execution.reconciled")),
                "proposal":self.state.proposals.iter().rev().find(|p| p.mode==self.state.mode && p.scope_id==scope && p.decision_id.as_deref()==self.state.last_decision.as_ref().and_then(|v| v["id"].as_str())).map(|p|json!({"id":p.id,"decision_id":p.decision_id,"position_id":p.position_id,"status":p.status,"notional":p.notional,"reason":p.reason})),
                "pending":self.state.live.pending,
                "fill_model":if self.state.mode==TradingMode::DryRun { "live quote output minus configured slippage; estimated fees; no on-chain execution" } else { "confirmed on-chain balance deltas" }
            }));
        }
        if self.state.events.len() > 150 {
            self.state.events.remove(0);
        }
    }
    fn persist(&mut self, receipt: Option<(&str, Option<&str>)>) -> Result<(), String> {
        let save = || -> Result<(), String> {
            let tx = self
                .db
                .unchecked_transaction()
                .map_err(|_| "Cannot begin durable transaction")?;
            let payload =
                serde_json::to_string(&self.state).map_err(|_| "Cannot serialize runtime")?;
            tx.execute("INSERT INTO runtime_state(id,payload) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", [&payload]).map_err(|_| "Cannot persist runtime")?;
            for event in &self.state.events {
                tx.execute(
                    "INSERT OR IGNORE INTO audit_events(sequence,payload) VALUES(?1,?2)",
                    params![
                        event.sequence,
                        serde_json::to_string(event).map_err(|_| "Cannot serialize event")?
                    ],
                )
                .map_err(|_| "Cannot append audit")?;
            }
            for session in self
                .state
                .archived_sessions
                .iter()
                .chain(self.state.session.iter())
            {
                tx.execute("INSERT INTO dry_run_sessions(id,payload) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", params![session.id,serde_json::to_string(session).map_err(|_| "Cannot serialize session")?]).map_err(|_| "Cannot persist session")?;
            }
            if let Some((request_id, error)) = receipt {
                tx.execute(
                    "INSERT OR IGNORE INTO command_receipts(id,error) VALUES(?1,?2)",
                    params![request_id, error],
                )
                .map_err(|_| "Cannot persist command receipt")?;
            }
            journal::enqueue(&tx, &self.journal_queue)?;
            tx.commit().map_err(|_| "Durable commit failed")?;
            journal::flush(&self.db, &self.journal_root)?;
            Ok(())
        };
        if let Err(error) = save() {
            self.journal_error = Some(error.clone());
            self.kill.store(true, Ordering::SeqCst);
            self.state.killed = true;
            self.state.paused = true;
            return Err(error);
        }
        self.journal_error = None;
        self.journal_queue.clear();
        Ok(())
    }
    pub fn portfolio(&self) -> Portfolio {
        let scope = self.scope();
        let positions: Vec<_> = self
            .state
            .positions
            .iter()
            .filter(|p| p.mode == self.state.mode && p.scope_id == scope)
            .collect();
        let exposure: Decimal = positions
            .iter()
            .filter(|p| p.status == "Open")
            .map(|p| money(p.quantity * p.mark_price))
            .sum();
        let cost: Decimal = positions
            .iter()
            .filter(|p| p.status == "Open")
            .map(|p| p.cost_basis)
            .sum();
        let realized_pnl = positions.iter().map(|p| p.realized_pnl).sum();
        let fees = self
            .state
            .orders
            .iter()
            .filter(|o| o.mode == self.state.mode && o.scope_id == scope)
            .map(|o| o.fee_usdc)
            .sum();
        let cash = if self.state.mode == TradingMode::DryRun {
            self.state
                .session
                .as_ref()
                .map(|s| s.balance)
                .unwrap_or_default()
        } else {
            self.state
                .live
                .wallet
                .as_ref()
                .map(|w| Decimal::from(w.usdc_atoms) / Decimal::from(1_000_000))
                .unwrap_or_default()
        };
        Portfolio {
            equity: money(cash + exposure),
            cash,
            exposure,
            realized_pnl,
            unrealized_pnl: money(exposure - cost),
            fees,
            open_positions: positions.iter().filter(|p| p.status == "Open").count(),
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        let dry = self.state.mode == TradingMode::DryRun;
        let authorized = !self.state.killed
            && self.state.live.unlocked
            && self.state.live.activation_expires_at > now();
        let row = |cap: &str, req: &str, status: &str, detail: &str| Readiness {
            capability: cap.into(),
            requirement: req.into(),
            status: status.into(),
            detail: detail.into(),
        };
        let mut readiness = vec![row(
            "Storage",
            "Required",
            "Ready",
            "SQLite WAL · durable commands and append-only audit",
        )];
        if dry {
            readiness.extend([
                row(
                    "Virtual account",
                    "Required",
                    if self.state.session.is_some() {
                        "Ready"
                    } else {
                        "Missing"
                    },
                    "Isolated DRY_RUN ledger",
                ),
                row(
                    "Wallet",
                    "Not required",
                    "Not required",
                    "Dry Run does not resolve a signer",
                ),
                row(
                    "Signing / submission",
                    "Prohibited",
                    "Prohibited",
                    "No on-chain execution capability is registered",
                ),
            ]);
        } else {
            readiness.extend([
                row(
                    "Wallet vault",
                    "Required",
                    if self.state.live.unlocked {
                        "Ready"
                    } else {
                        "Locked"
                    },
                    "Encrypted dedicated wallet · passphrase required",
                ),
                row(
                    "RPC / Jupiter",
                    "Required",
                    if self
                        .state
                        .live
                        .wallet
                        .as_ref()
                        .is_some_and(|w| now().saturating_sub(w.checked_at) <= 30)
                    {
                        "Ready"
                    } else {
                        "Missing"
                    },
                    self.state
                        .live
                        .readiness_error
                        .as_deref()
                        .unwrap_or("Refresh connections and confirmed balances"),
                ),
                row(
                    "Reconciliation",
                    "Required",
                    if self.state.live.pending.is_none() {
                        "Ready"
                    } else {
                        "Blocked"
                    },
                    "Unresolved signatures block additional transactions",
                ),
                row(
                    "Owner authorization",
                    "Required",
                    if authorized { "Ready" } else { "Locked" },
                    "Explicit activation expires after 15 minutes",
                ),
                row(
                    "Route policy",
                    "Required",
                    "Ready",
                    "USDC / wrapped SOL · Jupiter V2 · Orca Whirlpool only",
                ),
            ]);
        }
        readiness.extend([
            row("Live market feed", "Required", if self.state.market_error.is_some() { "Blocked" } else if self.state.markets.first().is_some_and(|m| now().saturating_sub(m.updated_at)<=30) { "Ready" } else { "Missing" }, self.state.market_error.as_deref().unwrap_or("Configure KAIROS_JUPITER_API_KEY and KAIROS_MARKET_RPC_URL on the Rust host")),
            row("AI decision provider", "Required for cycles", if self.services.ai_ready() { "Configured" } else { "Missing" }, "Set KAIROS_AI_URL, KAIROS_AI_MODEL and optional KAIROS_AI_API_KEY; decisions consume verified daily trade logs"),
            row("Daily trade journal", "Required", if self.journal_error.is_some() { "Blocked" } else { "Ready" }, &format!("{} / YYYY/MM/DD/*.json (Asia/Jakarta); SQLite outbox recovery",self.journal_root.display()))
        ]);
        Snapshot {
            state: self.state.clone(),
            portfolio: self.portfolio(),
            readiness,
            capabilities: self.state.mode.capabilities(authorized),
        }
    }
    pub async fn command(&mut self, command: Command) -> Result<Snapshot, String> {
        if Uuid::parse_str(&command.request_id).is_err() {
            return Err("A UUID request_id is required".into());
        }
        let previous: Option<Option<String>> = self
            .db
            .query_row(
                "SELECT error FROM command_receipts WHERE id=?1",
                [&command.request_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| "Cannot read command receipt")?;
        if let Some(error) = previous {
            return match error {
                Some(error) => Err(error),
                None => Ok(self.snapshot()),
            };
        }
        let result = self.apply(command.action, &command.request_id).await;
        if let Err(error) = &result {
            self.event("command.rejected", error, &command.request_id);
        }
        self.persist(Some((
            &command.request_id,
            result.as_ref().err().map(String::as_str),
        )))?;
        result.map(|_| self.snapshot())
    }
    async fn apply(&mut self, action: Action, correlation: &str) -> Result<(), String> {
        match action {
            Action::CreateSession {
                name,
                starting_balance,
                fee_bps,
                slippage_bps,
            } => {
                if self.state.mode != TradingMode::DryRun {
                    return Err("Select Dry Run before creating a virtual account".into());
                }
                if self
                    .state
                    .session
                    .as_ref()
                    .is_some_and(|s| !matches!(s.status.as_str(), "Stopped" | "Archived"))
                {
                    return Err("Stop the existing session before creating another".into());
                }
                if name.trim().is_empty()
                    || name.len() > 80
                    || starting_balance < Decimal::ONE
                    || starting_balance > Decimal::from(1_000_000)
                    || starting_balance.scale() > 6
                    || fee_bps > 100
                    || slippage_bps > self.state.policy.max_slippage_bps
                {
                    return Err("Invalid session configuration".into());
                }
                if let Some(mut old) = self.state.session.take() {
                    old.status = "Archived".into();
                    self.state.archived_sessions.push(old);
                }
                self.state.session = Some(DrySession {
                    id: id(),
                    parent_session_id: None,
                    name: name.trim().into(),
                    status: "Ready".into(),
                    starting_balance,
                    balance: starting_balance,
                    fee_bps,
                    slippage_bps,
                    created_at: now(),
                    policy: self.state.policy.clone(),
                    source: "Jupiter live market".into(),
                });
                self.state.paused = true;
                self.event(
                    "dry_run.created",
                    "Virtual account created without a wallet. Source: live Jupiter market data.",
                    correlation,
                );
            }
            Action::StartSession | Action::Resume => {
                if self.state.killed {
                    return Err(
                        "Kill switch is latched. Stop/reset Dry Run or reactivate Live explicitly"
                            .into(),
                    );
                }
                if self.state.mode == TradingMode::Live {
                    self.require_live()?;
                } else {
                    let session = self
                        .state
                        .session
                        .as_mut()
                        .ok_or("Create a Dry Run session first")?;
                    if !matches!(session.status.as_str(), "Ready" | "Paused" | "Running") {
                        return Err("Only a ready or paused session can start".into());
                    }
                    if session.source != "Jupiter live market" {
                        return Err(
                            "Reset the legacy replay session before using live market data".into(),
                        );
                    }
                    session.status = "Running".into();
                }
                self.state.paused = false;
                self.event(
                    "runtime.resumed",
                    "Entry pipeline resumed; deterministic exit supervision remains active.",
                    correlation,
                );
            }
            Action::Pause => {
                self.state.paused = true;
                if let Some(session) = self.state.session.as_mut() {
                    if session.status == "Running" {
                        session.status = "Paused".into();
                    }
                }
                self.event(
                    "runtime.paused",
                    "New entries paused. Existing position exit rules remain active.",
                    correlation,
                );
            }
            Action::Kill => {
                self.kill.store(true, Ordering::SeqCst);
                self.state.killed = true;
                self.state.paused = true;
                self.state.live.activation_expires_at = 0;
                for proposal in &mut self.state.proposals {
                    if proposal.status == "Pending" {
                        proposal.status = "Rejected".into();
                        proposal.reason = "Kill switch".into();
                    }
                }
                self.event("security.kill_switch", "Kill switch latched. Pending proposals cancelled; no new signing or submission.", correlation);
            }
            Action::StopSession => {
                if self.state.mode != TradingMode::DryRun {
                    return Err("Stop session is a Dry Run command".into());
                }
                self.state
                    .session
                    .as_mut()
                    .ok_or("No active session")?
                    .status = "Stopped".into();
                self.state.paused = true;
                self.event(
                    "dry_run.stopped",
                    "Session stopped; its history is retained.",
                    correlation,
                );
            }
            Action::ResetSession => {
                if self.state.mode != TradingMode::DryRun {
                    return Err("Cannot reset a Live ledger".into());
                }
                let old = self.state.session.as_ref().ok_or("No session to reset")?;
                if old.status != "Stopped" {
                    return Err("Stop the session before resetting it".into());
                }
                let mut next = old.clone();
                next.parent_session_id = Some(old.id.clone());
                next.id = id();
                next.balance = next.starting_balance;
                next.status = "Ready".into();
                next.created_at = now();
                next.source = "Jupiter live market".into();
                let mut old = self.state.session.replace(next).unwrap();
                old.status = "Archived".into();
                self.state.archived_sessions.push(old);
                self.state.killed = false;
                self.kill.store(false, Ordering::SeqCst);
                self.event("dry_run.reset", "Created a new virtual account with reset lineage. Previous ledger and audit retained.", correlation);
            }
            Action::SetMode { mode } => {
                if mode == self.state.mode {
                    return Ok(());
                }
                if self.state.live.pending.is_some() {
                    return Err("Reconcile pending live transaction before switching mode".into());
                }
                if self.state.mode == TradingMode::Live && self.portfolio().open_positions > 0 {
                    return Err("Close live positions before switching execution mode".into());
                }
                self.live = None;
                self.state.live.unlocked = false;
                self.state.live.activation_expires_at = 0;
                self.state.paused = true;
                if let Some(session) = self.state.session.as_mut() {
                    if session.status == "Running" {
                        session.status = "Paused".into();
                    }
                }
                self.state.mode = mode;
                self.state.last_decision = None;
                self.state.last_quote = None;
                self.state.agents = initial_agents(mode);
                self.event(
                    "runtime.mode_changed",
                    "Execution context changed. Signer dropped; ledgers remain isolated.",
                    correlation,
                );
            }
            Action::RunCycle => {
                self.run_cycle(correlation).await?;
            }
            Action::ApproveProposal { id } => {
                self.approve(&id, correlation).await?;
            }
            Action::RejectProposal { id } => {
                let scope = self.scope();
                let mode = self.state.mode;
                let proposal = self
                    .state
                    .proposals
                    .iter_mut()
                    .find(|p| p.id == id && p.scope_id == scope && p.mode == mode)
                    .ok_or("Proposal not found in current scope")?;
                if proposal.status != "Pending" {
                    return Err("Proposal is already terminal".into());
                }
                proposal.status = "Rejected".into();
                proposal.reason = "Rejected by owner".into();
                self.event(
                    "proposal.rejected",
                    "Proposal rejected by owner.",
                    correlation,
                );
            }
            Action::ClosePosition { id } => {
                self.close_position(&id, "Manual close", correlation)
                    .await?;
            }
            Action::UpdatePolicy { policy } => {
                policy.validate()?;
                if !self.state.paused {
                    return Err("Pause entries before editing risk policy".into());
                }
                if self.state.live.pending.is_some() {
                    return Err("Cannot change policy while a transaction is pending".into());
                }
                self.state.policy = policy;
                self.state.live.activation_expires_at = 0;
                self.event("risk.policy_updated", "Risk policy updated; open positions retain their original exit plan. Live activation invalidated.", correlation);
            }
            Action::ConfigureLive {
                rpc_url,
                secondary_rpc_url,
                jupiter_api_key,
                keypair_json,
                passphrase,
            } => {
                let passphrase = Zeroizing::new(passphrase);
                let keypair_json = keypair_json.map(Zeroizing::new);
                kairos_live::validate_endpoint(&rpc_url)?;
                kairos_live::validate_endpoint(&secondary_rpc_url)?;
                if rpc_url == secondary_rpc_url
                    || jupiter_api_key.trim().is_empty()
                    || jupiter_api_key.len() > 512
                {
                    return Err(
                        "Two distinct RPC endpoints and a Jupiter API key are required".into(),
                    );
                }
                use kairos_live::new_keypair_bytes;
                let keypair = match keypair_json {
                    Some(bytes) if !bytes.trim().is_empty() => {
                        serde_json::from_str::<Vec<u8>>(&bytes)
                            .map_err(|_| "Keypair must be a 64-byte Solana JSON array")?
                    }
                    _ => new_keypair_bytes(),
                };
                let public_key = vault::create(
                    &self.directory.join("vault.json"),
                    vault::Credentials {
                        rpc_url,
                        secondary_rpc_url,
                        jupiter_api_key,
                        keypair,
                    },
                    &passphrase,
                )?;
                self.state.live.public_key = Some(public_key);
                self.event(
                    "wallet.configured",
                    "Dedicated wallet and provider credentials encrypted. Secrets are write-only.",
                    correlation,
                );
            }
            Action::UnlockWallet { passphrase } => {
                let passphrase = Zeroizing::new(passphrase);
                if self.state.mode != TradingMode::Live {
                    return Err(
                        "ModeCapabilityViolation: signer cannot be resolved in Dry Run".into(),
                    );
                }
                if self.live.is_none() {
                    self.live = Some(LiveExecutor::unlock(
                        &self.directory,
                        &passphrase,
                        self.kill.clone(),
                    )?);
                }
                self.state.live.unlocked = true;
                self.event(
                    "wallet.unlocked",
                    "Signer unlocked; live activation is still required.",
                    correlation,
                );
            }
            Action::LockWallet => {
                self.live = None;
                self.state.live.unlocked = false;
                self.state.live.activation_expires_at = 0;
                self.state.paused = true;
                self.event(
                    "wallet.locked",
                    "Signer locked and execution authorization revoked.",
                    correlation,
                );
            }
            Action::RefreshWallet => {
                self.refresh_wallet().await?;
                self.event(
                    "wallet.refreshed",
                    "Confirmed balances and independent RPC cluster/slot checks refreshed.",
                    correlation,
                );
            }
            Action::ActivateLive { confirmation } => {
                if self.state.mode != TradingMode::Live || confirmation != "ACTIVATE LIVE" {
                    return Err(
                        "Enter ACTIVATE LIVE in Live mode to authorize a 15-minute session".into(),
                    );
                }
                if self.state.live.pending.is_some() {
                    return Err("Reconcile pending transaction first".into());
                }
                self.state.policy.validate()?;
                self.refresh_wallet().await?;
                let wallet = self
                    .state
                    .live
                    .wallet
                    .as_ref()
                    .ok_or("Wallet unavailable")?;
                if wallet.sol_lamports
                    < kairos_live::FEE_RESERVE_LAMPORTS + kairos_live::MAX_FEE_LAMPORTS
                {
                    return Err("SOL fee reserve is below 0.0201 SOL".into());
                }
                if wallet.usdc_atoms
                    < (self.state.policy.max_trade_usdc * Decimal::from(1_000_000))
                        .to_u64()
                        .ok_or("Amount conversion failed")?
                {
                    return Err("USDC balance is below maximum trade notional".into());
                }
                // Operations gate is explicit and external to UI; never inferred from a wallet being funded.
                if !self
                    .directory
                    .join("live-validation-approved.txt")
                    .is_file()
                {
                    return Err("Live validation gate missing: complete the README live validation checklist and provision live-validation-approved.txt".into());
                }
                self.state.live.activation_expires_at = now() + 900;
                self.state.paused = false;
                self.state.killed = false;
                self.kill.store(false, Ordering::SeqCst);
                self.event("security.live_activated", "Owner authorized live execution for 15 minutes. Each proposal still requires approval.", correlation);
            }
            Action::Reconcile => {
                self.reconcile(correlation).await?;
            }
        }
        Ok(())
    }
    fn require_live(&self) -> Result<(), String> {
        if self.state.mode != TradingMode::Live
            || self.live.is_none()
            || self.state.live.activation_expires_at <= now()
            || self.kill.load(Ordering::SeqCst)
        {
            return Err(
                "Live execution requires an unlocked signer and unexpired owner activation".into(),
            );
        }
        if self.state.live.pending.is_some() {
            return Err("Unresolved transaction blocks additional execution".into());
        }
        Ok(())
    }
    async fn refresh_wallet(&mut self) -> Result<(), String> {
        let live = self
            .live
            .as_ref()
            .ok_or("Unlock the wallet in Live mode first")?;
        match live.wallet().await {
            Ok(wallet) => {
                self.state.live.wallet = Some(wallet);
                self.state.live.readiness_error = None;
                Ok(())
            }
            Err(error) => {
                self.state.live.readiness_error = Some(error.clone());
                Err(error)
            }
        }
    }
    async fn run_cycle(&mut self, correlation: &str) -> Result<(), String> {
        let cycle = id();
        self.state.agents = initial_agents(self.state.mode);
        self.state.last_decision = None;
        for agent in &mut self.state.agents {
            agent.cycle_id = Some(cycle.clone());
            agent.status = "Waiting".into();
            agent.model = self.services.model.clone();
        }
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(75),
            self.run_agent_cycle(correlation, &cycle),
        )
        .await
        .unwrap_or_else(|_| Err("Agent cycle exceeded 75-second deadline".into()));
        if let Err(error) = &result {
            for agent in &mut self.state.agents {
                if agent.status == "Running" {
                    agent.status = "Failed".into();
                    agent.output = error.clone();
                    agent.completed_at = Some(now());
                } else if agent.status == "Waiting" {
                    agent.status = "Skipped".into();
                    agent.output = "Dependency did not complete; no analysis fabricated.".into();
                }
            }
            self.event("agent.cycle.failed", error, correlation);
        }
        result
    }
    fn agent_started(&mut self, index: usize, correlation: &str) -> Result<(), String> {
        if self.kill.load(Ordering::SeqCst) {
            return Err("Agent cycle cancelled by kill switch".into());
        }
        self.state.agents[index].status = "Running".into();
        self.state.agents[index].output =
            "Evaluating current evidence and verified trade memory.".into();
        self.event(
            "agent.started",
            &format!("{} started", self.state.agents[index].name),
            correlation,
        );
        self.persist(None)
    }
    fn agent_completed(
        &mut self,
        index: usize,
        output: &impl serde::Serialize,
        correlation: &str,
    ) -> Result<(), String> {
        self.state.agents[index].output =
            serde_json::to_string_pretty(output).map_err(|_| "Cannot serialize agent evidence")?;
        self.state.agents[index].status = "Complete".into();
        self.state.agents[index].completed_at = Some(now());
        self.event(
            "agent.completed",
            &format!("{} completed", self.state.agents[index].name),
            correlation,
        );
        self.persist(None)
    }
    fn analyst_result(
        &mut self,
        index: usize,
        result: Result<AgentReport, String>,
        correlation: &str,
    ) -> Result<AgentReport, String> {
        match result {
            Ok(report) => {
                self.agent_completed(index, &report, correlation)?;
                Ok(report)
            }
            Err(error) => {
                self.state.agents[index].status = "Failed".into();
                self.state.agents[index].output = error.clone();
                self.state.agents[index].completed_at = Some(now());
                self.event(
                    "agent.failed",
                    &format!("{}: {error}", self.state.agents[index].name),
                    correlation,
                );
                self.persist(None)?;
                Err(error)
            }
        }
    }
    async fn supervise_inference<T>(
        &mut self,
        inference: impl std::future::Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        tokio::pin!(inference);
        let period = std::time::Duration::from_secs(5);
        let mut timer = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                result=&mut inference => return result,
                _=timer.tick()=> {
                    self.tick().await?;
                    if self.kill.load(Ordering::SeqCst) || self.state.paused { return Err("Agent cycle cancelled while supervising positions".into()); }
                }
            }
        }
    }
    async fn run_agent_cycle(&mut self, correlation: &str, cycle: &str) -> Result<(), String> {
        // ponytail: bounded snapshot storage for the preview; migrate to paginated tables for long-running production.
        if self.state.proposals.len() >= 5_000 {
            return Err("Preview history limit reached (5,000 proposals). Archive/export the runtime before starting a new one".into());
        }
        if self.state.paused || self.state.killed {
            return Err("Start or resume the runtime before running a cycle".into());
        }
        if self.state.mode == TradingMode::DryRun
            && self
                .state
                .session
                .as_ref()
                .is_none_or(|s| s.status != "Running")
        {
            return Err("A running Dry Run session is required".into());
        }
        if self.state.mode == TradingMode::Live {
            self.require_live()?;
            self.refresh_wallet().await?;
        }
        self.agent_started(0, correlation)?;
        self.refresh_market().await?;
        self.persist(None)?;
        let memory = journal::memory(&self.db, &self.journal_root)?;
        let scope = self.scope();
        let positions: Vec<_> = self
            .state
            .positions
            .iter()
            .filter(|p| p.mode == self.state.mode && p.scope_id == scope && p.status == "Open")
            .cloned()
            .collect();
        let mut context = json!({"cycle_id":cycle,"mode":self.state.mode,"scope_id":scope,"market":self.state.markets,"market_slot":self.state.market_slot,"portfolio":self.portfolio(),"risk_policy":self.state.policy,"open_positions":positions,"trade_history":memory});
        let services = self.services.clone();
        let input = context.clone();
        let orchestration = self
            .supervise_inference(async move { services.analyze("orchestrator", &input).await })
            .await?;
        self.agent_completed(0, &orchestration, correlation)?;
        context["orchestration"] = json!(orchestration);
        context["trade_history"] = json!(journal::memory(&self.db, &self.journal_root)?);
        self.agent_started(1, correlation)?;
        self.agent_started(2, correlation)?;
        self.refresh_market().await?;
        context["market"] = json!(self.state.markets);
        context["market_slot"] = json!(self.state.market_slot);
        context["onchain"] = self.services.onchain(self.state.market_slot).await?;
        context["portfolio"] = json!(self.portfolio());
        context["open_positions"] = json!(self
            .state
            .positions
            .iter()
            .filter(|p| p.mode == self.state.mode && p.scope_id == scope && p.status == "Open")
            .collect::<Vec<_>>());
        let services = self.services.clone();
        let input = context.clone();
        let (market, onchain) = self
            .supervise_inference(async move {
                Ok(tokio::join!(
                    services.analyze("market_analyst", &input),
                    services.analyze("onchain_analyst", &input)
                ))
            })
            .await?;
        // Persist each independent result even if its peer fails.
        let market = self.analyst_result(1, market, correlation);
        let onchain = self.analyst_result(2, onchain, correlation);
        let (market, onchain) = (market?, onchain?);
        let blocked = [&orchestration, &market, &onchain]
            .iter()
            .any(|r| r.assessment == Assessment::Block);
        context["agent_reports"] =
            json!({"orchestrator":orchestration,"market_analyst":market,"onchain_analyst":onchain});
        context["analysis_market"] = context["market"].clone();
        context["analysis_onchain"] = context["onchain"].clone();
        self.agent_started(3, correlation)?;
        // The final model sees fresh evidence as well as the snapshots its analysts used.
        self.refresh_market().await?;
        context["market"] = json!(self.state.markets);
        context["market_slot"] = json!(self.state.market_slot);
        context["onchain"] = self.services.onchain(self.state.market_slot).await?;
        context["portfolio"] = json!(self.portfolio());
        context["open_positions"] = json!(self
            .state
            .positions
            .iter()
            .filter(|p| p.mode == self.state.mode && p.scope_id == scope && p.status == "Open")
            .collect::<Vec<_>>());
        context["trade_history"] = json!(journal::memory(&self.db, &self.journal_root)?);
        let services = self.services.clone();
        let input = context.clone();
        let decision = self
            .supervise_inference(async move { services.decide(&input).await })
            .await?;
        if blocked && decision.action != DecisionAction::Hold {
            return Err("Strategy cannot override an agent block; HOLD required".into());
        }
        match decision.action {
            DecisionAction::Open
                if decision.position_id.is_none()
                    && decision.notional_usdc > Decimal::ZERO
                    && decision.notional_usdc <= self.state.policy.max_trade_usdc => {}
            DecisionAction::Close
                if decision.notional_usdc == Decimal::ZERO
                    && self.state.positions.iter().any(|p| {
                        p.mode == self.state.mode
                            && p.scope_id == scope
                            && p.status == "Open"
                            && Some(&p.id) == decision.position_id.as_ref()
                    }) => {}
            DecisionAction::Hold
                if decision.position_id.is_none() && decision.notional_usdc == Decimal::ZERO => {}
            _ => return Err("AI decision violates position scope or risk bounds".into()),
        }
        if now().saturating_sub(
            context["market"][0]["updated_at"]
                .as_u64()
                .unwrap_or_default(),
        ) > 30
        {
            return Err("Strategy input expired during inference".into());
        }
        // Model latency cannot make stale evidence eligible for approval.
        if self.kill.load(Ordering::SeqCst)
            || self.state.paused
            || self
                .state
                .markets
                .first()
                .is_none_or(|m| now().saturating_sub(m.updated_at) > 30)
        {
            return Err("Decision cancelled: killed or market evidence stale".into());
        }
        self.state.cycle_count += 1;
        let memory_ids: Vec<_> = context["trade_history"]
            .as_array()
            .ok_or("Trade memory context missing")?
            .iter()
            .map(|r| r["event"]["id"].clone())
            .collect();
        self.state.last_decision = Some(
            json!({"id":cycle,"model":self.services.model,"received_at":now(),"memory_event_ids":memory_ids,"agent_reports":context["agent_reports"],"analysis_market":context["analysis_market"],"analysis_onchain":context["analysis_onchain"],"onchain":context["onchain"],"result":decision}),
        );
        self.agent_completed(3, &decision, correlation)?;
        if decision.action != DecisionAction::Hold {
            self.state.proposals.push(Proposal {
                id: id(),
                scope_id: self.scope(),
                mode: self.state.mode,
                symbol: "SOL".into(),
                position_id: decision.position_id,
                decision_evidence: self.state.last_decision.clone(),
                decision_id: Some(cycle.into()),
                notional: decision.notional_usdc,
                thesis: format!(
                    "{}: {} Lessons: {}",
                    if decision.action == DecisionAction::Open {
                        "OPEN"
                    } else {
                        "CLOSE"
                    },
                    decision.rationale,
                    decision.lessons
                ),
                status: "Pending".into(),
                reason: "Awaiting owner approval".into(),
                created_at: now(),
                expires_at: now() + 60,
            });
        }
        self.event("agent.cycle.completed",if decision.action==DecisionAction::Hold { "AI chose HOLD after evaluating live market and trade memory." } else { "AI proposal created from live market and verified daily trade memory; owner approval required." },correlation);
        Ok(())
    }
    async fn refresh_market(&mut self) -> Result<(), String> {
        let (price, slot) = match self.services.market().await {
            Ok(value) => value,
            Err(error) => {
                self.state.market_error = Some(error.clone());
                return Err(error);
            }
        };
        let previous = self.state.markets.first();
        let change_bps = previous
            .map(|m| {
                ((price / m.price - Decimal::ONE) * Decimal::from(10_000))
                    .to_i32()
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let mut history = previous.map(|m| m.history.clone()).unwrap_or_default();
        history.push(price);
        if history.len() > 60 {
            history.remove(0);
        }
        self.state.markets = vec![Market {
            symbol: "SOL".into(),
            name: "Wrapped Solana".into(),
            price,
            change_bps,
            history,
            source: "Jupiter live market".into(),
            updated_at: now(),
        }];
        self.state.market_slot = slot;
        self.state.market_error = None;
        let scope = self.scope();
        for p in self.state.positions.iter_mut().filter(|p| {
            p.mode == self.state.mode
                && p.scope_id == scope
                && p.status == "Open"
                && p.symbol == "SOL"
        }) {
            p.mark_price = price;
            p.high_price = p.high_price.max(price);
        }
        Ok(())
    }
    async fn approve(&mut self, proposal_id: &str, correlation: &str) -> Result<(), String> {
        let scope = self.scope();
        let index = self
            .state
            .proposals
            .iter()
            .position(|p| p.id == proposal_id && p.scope_id == scope && p.mode == self.state.mode)
            .ok_or("Proposal not found in current mode/account")?;
        let proposal = self.state.proposals[index].clone();
        if proposal.status != "Pending" {
            return Err("Proposal has already been consumed".into());
        }
        if now() > proposal.expires_at {
            self.state.proposals[index].status = "Expired".into();
            return Err("Proposal expired; run a new cycle".into());
        }
        self.state.last_quote = None;
        self.state.last_decision = proposal.decision_evidence.clone();
        self.event(
            "execution.requested",
            &format!(
                "Approve proposal {proposal_id}; decision {:?}",
                proposal.decision_id
            ),
            correlation,
        );
        self.persist(None)?;
        if let Some(position_id) = &proposal.position_id {
            self.state.proposals[index].status = "Approved".into();
            self.persist(None)?;
            if let Err(error) = self
                .close_position(position_id, "AI close approved by owner", correlation)
                .await
            {
                self.state.proposals[index].reason =
                    format!("Close request failed or pending: {error}");
                return Err(error);
            }
            self.state.proposals[index].reason = if self.state.mode == TradingMode::Live {
                "AI close submitted; confirmation pending"
            } else {
                "AI close executed after owner approval"
            }
            .into();
            return Ok(());
        }
        self.refresh_market().await?;
        if self.state.mode == TradingMode::Live {
            self.require_live()?;
            self.refresh_wallet().await?;
        }
        let quote = self
            .services
            .quote(
                true,
                (proposal.notional * Decimal::from(1_000_000))
                    .to_u64()
                    .ok_or("Invalid notional")?,
                self.state.policy.max_slippage_bps,
                self.state.markets[0].price,
            )
            .await?;
        self.state.last_quote = Some(quote.clone());
        let portfolio = self.portfolio();
        let market = self
            .state
            .markets
            .iter()
            .find(|m| m.symbol == proposal.symbol)
            .ok_or("Market unavailable")?
            .clone();
        let fee = if self.state.mode == TradingMode::DryRun {
            money(proposal.notional * bps(self.state.session.as_ref().ok_or("No session")?.fee_bps))
        } else {
            Decimal::ZERO
        };
        let input = RiskInput {
            mode: self.state.mode,
            scope_id: scope.clone(),
            paused: self.state.paused || self.state.killed,
            data_fresh: now().saturating_sub(market.updated_at) <= 30,
            token_safe: true,
            notional: proposal.notional,
            fee,
            open_positions: portfolio.open_positions,
            exposure: portfolio.exposure,
            realized_pnl: portfolio.realized_pnl,
            available_balance: portfolio.cash,
        };
        let intent = match self.state.policy.approve(&input) {
            Ok(intent) => intent,
            Err(error) => {
                self.state.proposals[index].status = "Rejected".into();
                self.state.proposals[index].reason = error.clone();
                self.event("risk.rejected", &error, correlation);
                return Err(error);
            }
        };
        if self.state.mode == TradingMode::DryRun {
            if self.kill.load(Ordering::SeqCst) {
                return Err("Kill switch blocked virtual entry".into());
            }
            self.state.last_quote = Some(quote.clone());
            if now().saturating_sub(self.state.markets[0].updated_at) > 30 {
                return Err("Entry market evidence is stale".into());
            }
            let session = self.state.session.as_mut().ok_or("No session")?;
            if session.status != "Running" {
                return Err("Session is not running".into());
            }
            let quantity = (Decimal::from(quote.output_atoms)
                * (Decimal::ONE - bps(session.slippage_bps)))
            .floor()
                / Decimal::from(1_000_000_000);
            if quantity <= Decimal::ZERO {
                return Err("Virtual fill rounded to zero".into());
            }
            let spent = intent.notional();
            let fill_price = spent / quantity;
            session.balance = money(session.balance - spent - fee);
            let position_id = id();
            self.state.positions.push(Position {
                id: position_id.clone(),
                scope_id: scope.clone(),
                mode: TradingMode::DryRun,
                symbol: proposal.symbol.clone(),
                quantity,
                entry_price: fill_price,
                mark_price: market.price,
                high_price: fill_price,
                cost_basis: spent + fee,
                realized_pnl: Decimal::ZERO,
                status: "Open".into(),
                opened_at: now(),
                exit_reason: None,
                policy: self.state.policy.clone(),
            });
            self.state.orders.push(Order {
                position_id: Some(position_id),
                decision_id: proposal.decision_id.clone(),
                id: id(),
                scope_id: scope,
                mode: TradingMode::DryRun,
                symbol: proposal.symbol,
                side: "Buy".into(),
                quantity,
                price: fill_price,
                fee_usdc: fee,
                fee_lamports: 0,
                signature: None,
                timestamp: now(),
                reason: "Simulated fill from fresh Jupiter quote".into(),
            });
            self.state.proposals[index].status = "Approved".into();
            self.event("execution.virtual_filled", "RiskApproved → Simulating → VirtualFilled → VirtualReconciled. No signer or submitter.", correlation);
        } else {
            self.require_live()?;
            let amount = (proposal.notional * Decimal::from(1_000_000))
                .to_u64()
                .ok_or("Invalid input amount")?;
            let prepared = self
                .live
                .as_ref()
                .unwrap()
                .prepare(intent, true, amount, self.state.policy.max_slippage_bps)
                .await?;
            self.state.last_quote = Some(prepared.quote.clone());
            self.require_live()?;
            // Persist authorization consumption before any signature can be submitted.
            self.state.proposals[index].status = "Approved".into();
            self.state.proposals[index].reason = "RiskApproved; transaction simulated".into();
            self.event(
                "risk.approved",
                "Live risk intent approved and unsigned transaction simulation validated.",
                correlation,
            );
            self.persist(None)?;
            let signed = self.live.as_ref().unwrap().sign(prepared)?;
            self.state.live.pending = Some(signed.pending.clone());
            self.state.live.pending_decision = self.state.last_decision.clone();
            self.state.live.pending_position_id = None;
            self.event(
                "execution.pending",
                "Signature durably recorded before network submission.",
                correlation,
            );
            self.persist(None)?;
            let result = self.live.as_ref().unwrap().submit(signed).await;
            if let Err(error) = result {
                self.event("execution.unknown", &error, correlation);
                return Err("Submission result uncertain. Reconcile the stored signature; do not create a replacement trade".into());
            }
            self.event(
                "execution.submitted",
                "RPC accepted the signed transaction. Confirmation and reconciliation are pending.",
                correlation,
            );
        }
        self.state.proposals[index].status = "Approved".into();
        self.state.proposals[index].reason = "Deterministic risk checks passed".into();
        Ok(())
    }
    async fn close_position(
        &mut self,
        position_id: &str,
        reason: &str,
        correlation: &str,
    ) -> Result<(), String> {
        let scope = self.scope();
        let mode = self.state.mode;
        let index = self
            .state
            .positions
            .iter()
            .position(|p| p.id == position_id && p.scope_id == scope && p.mode == mode)
            .ok_or("Position not found in current mode/account")?;
        let p = self.state.positions[index].clone();
        if p.status != "Open" {
            return Err("Position is already closed or pending".into());
        }
        if p.symbol != "SOL" {
            return Err(
                "Legacy replay position cannot use the live feed; archive/reset its session".into(),
            );
        }
        if reason != "AI close approved by owner" {
            self.state.last_decision = None;
        }
        self.state.last_quote = None;
        self.event(
            "execution.exit_requested",
            &format!("Close position {position_id}: {reason}"),
            correlation,
        );
        self.persist(None)?;
        self.refresh_market().await?;
        if mode == TradingMode::DryRun {
            let amount = (p.quantity * Decimal::from(1_000_000_000))
                .to_u64()
                .ok_or("Invalid position quantity")?;
            let quote = self
                .services
                .quote(
                    false,
                    amount,
                    self.state.policy.max_slippage_bps,
                    self.state.markets[0].price,
                )
                .await?;
            if now().saturating_sub(self.state.markets[0].updated_at) > 30 {
                return Err("Exit market evidence is stale".into());
            }
            self.state.last_quote = Some(quote.clone());
            let session = self.state.session.as_mut().ok_or("No session")?;
            let gross = (Decimal::from(quote.output_atoms)
                * (Decimal::ONE - bps(session.slippage_bps)))
            .floor()
                / Decimal::from(1_000_000);
            let price = gross / p.quantity;
            let fee = money(gross * bps(session.fee_bps));
            session.balance = money(session.balance + gross - fee);
            self.state.positions[index].realized_pnl = money(gross - fee - p.cost_basis);
            self.state.positions[index].status = "Closed".into();
            self.state.positions[index].exit_reason = Some(reason.into());
            self.state.orders.push(Order {
                position_id: Some(p.id.clone()),
                decision_id: self
                    .state
                    .last_decision
                    .as_ref()
                    .and_then(|v| v["id"].as_str())
                    .map(str::to_string),
                id: id(),
                scope_id: scope,
                mode,
                symbol: p.symbol,
                side: "Sell".into(),
                quantity: p.quantity,
                price,
                fee_usdc: fee,
                fee_lamports: 0,
                signature: None,
                timestamp: now(),
                reason: reason.into(),
            });
            self.event(
                "position.closed",
                &format!("Simulated position closed: {reason}. Virtual ledger reconciled."),
                correlation,
            );
        } else {
            self.require_live()?;
            self.refresh_wallet().await?;
            let wallet = self.state.live.wallet.as_ref().unwrap();
            let amount = (p.quantity * Decimal::from(1_000_000_000))
                .to_u64()
                .ok_or("Invalid exit quantity")?;
            if amount > wallet.wsol_atoms {
                return Err("Position amount exceeds confirmed wrapped SOL balance".into());
            }
            let intent = self
                .state
                .policy
                .approve_exit(mode, &scope, amount, wallet.wsol_atoms)?;
            let prepared = self
                .live
                .as_ref()
                .unwrap()
                .prepare(intent, false, amount, self.state.policy.max_slippage_bps)
                .await?;
            self.require_live()?;
            self.state.last_quote = Some(prepared.quote.clone());
            let signed = self.live.as_ref().unwrap().sign(prepared)?;
            self.state.live.pending = Some(signed.pending.clone());
            self.state.live.pending_decision = self.state.last_decision.clone();
            self.state.live.pending_position_id = Some(p.id.clone());
            self.state.positions[index].status = "ExitPending".into();
            self.state.positions[index].exit_reason = Some(reason.into());
            self.event(
                "execution.exit_pending",
                &format!(
                    "Risk-reducing exit prepared: {reason}. Signature recorded before submission."
                ),
                correlation,
            );
            self.persist(None)?;
            self.live.as_ref().unwrap().submit(signed).await?;
        }
        Ok(())
    }
    async fn reconcile(&mut self, correlation: &str) -> Result<(), String> {
        let pending = self
            .state
            .live
            .pending
            .clone()
            .ok_or("No pending live transaction")?;
        let live = self.live.as_ref().ok_or("Unlock wallet to reconcile")?;
        let Some(fill) = live.reconcile(&pending).await? else {
            return Err("Transaction is not confirmed yet; pending state retained".into());
        };
        self.record_confirmed_fill(pending, fill, correlation)?;
        self.refresh_wallet().await?;
        Ok(())
    }
    fn record_confirmed_fill(
        &mut self,
        pending: kairos_live::PendingTrade,
        fill: kairos_live::ConfirmedFill,
        correlation: &str,
    ) -> Result<(), String> {
        self.state.last_decision = self.state.live.pending_decision.clone();
        self.state.last_quote = Some(pending.quote.clone());
        let buy = pending.quote.input_mint == USDC;
        let position_id = if buy {
            id()
        } else {
            self.state
                .live
                .pending_position_id
                .clone()
                .ok_or("Missing exit position")?
        };
        let quantity = Decimal::from(if buy {
            fill.output_atoms
        } else {
            fill.input_atoms
        }) / Decimal::from(1_000_000_000);
        let notional = Decimal::from(if buy {
            fill.input_atoms
        } else {
            fill.output_atoms
        }) / Decimal::from(1_000_000);
        let price = notional / quantity;
        let fee_usdc =
            money(Decimal::from(fill.fee_lamports) / Decimal::from(1_000_000_000) * price);
        if buy {
            self.state.positions.push(Position {
                id: position_id.clone(),
                scope_id: pending.scope_id.clone(),
                mode: TradingMode::Live,
                symbol: "SOL".into(),
                quantity,
                entry_price: price,
                mark_price: price,
                high_price: price,
                cost_basis: notional + fee_usdc,
                realized_pnl: Decimal::ZERO,
                status: "Open".into(),
                opened_at: now(),
                exit_reason: None,
                policy: self.state.policy.clone(),
            });
        } else {
            let p = self
                .state
                .positions
                .iter_mut()
                .find(|p| Some(&p.id) == self.state.live.pending_position_id.as_ref())
                .ok_or("Pending exit position missing; reconciliation required")?;
            p.realized_pnl = money(notional - fee_usdc - p.cost_basis);
            p.status = "Closed".into();
            p.exit_reason
                .get_or_insert_with(|| "Confirmed live exit".into());
        }
        self.state.orders.push(Order {
            position_id: Some(position_id),
            decision_id: self
                .state
                .last_decision
                .as_ref()
                .and_then(|v| v["id"].as_str())
                .map(str::to_string),
            id: id(),
            scope_id: pending.scope_id,
            mode: TradingMode::Live,
            symbol: "SOL".into(),
            side: if buy { "Buy".into() } else { "Sell".into() },
            quantity,
            price,
            fee_usdc,
            fee_lamports: fill.fee_lamports,
            signature: Some(fill.signature),
            timestamp: now(),
            reason: "Confirmed and reconciled from actual on-chain balances".into(),
        });
        self.state.live.pending = None;
        self.state.live.pending_decision = None;
        self.state.live.pending_position_id = None;
        self.event(
            "execution.reconciled",
            "Confirmed transaction reconciled against actual token balance deltas.",
            correlation,
        );
        self.persist(None)?;
        Ok(())
    }
    /// Bounded host timer. Pausing entries never suspends deterministic exit checks.
    pub async fn tick(&mut self) -> Result<(), String> {
        for proposal in &mut self.state.proposals {
            if proposal.status == "Pending" && proposal.expires_at < now() {
                proposal.status = "Expired".into();
                proposal.reason = "Approval window expired".into();
            }
        }
        if self.state.mode == TradingMode::Live
            && self.live.is_some()
            && self.state.live.pending.is_some()
        {
            if let Err(error) = self.reconcile("supervisor").await {
                self.state.live.readiness_error = Some(error);
            }
            return self.persist(None);
        }
        if let Err(error) = self.refresh_market().await {
            self.event("market.unavailable", &error, "supervisor");
            self.persist(None)?;
            return Err(error);
        }
        if self.state.mode == TradingMode::Live
            && (self.live.is_none() || self.state.live.activation_expires_at <= now())
        {
            self.state.paused = true;
            return self.persist(None);
        }
        if self.state.mode == TradingMode::DryRun
            && self.state.session.as_ref().is_none_or(|s| {
                !matches!(s.status.as_str(), "Running" | "Paused")
                    || s.source != "Jupiter live market"
            })
        {
            return self.persist(None);
        }
        let scope = self.scope();
        let exits: Vec<_> = self
            .state
            .positions
            .iter()
            .filter(|p| p.mode == self.state.mode && p.scope_id == scope && p.status == "Open")
            .filter_map(|p| {
                let reason = if p.mark_price
                    <= p.entry_price * (Decimal::ONE - bps(p.policy.stop_loss_bps))
                {
                    Some("Stop loss")
                } else if p.mark_price
                    >= p.entry_price * (Decimal::ONE + bps(p.policy.take_profit_bps))
                {
                    Some("Take profit")
                } else if p.mark_price
                    <= p.high_price * (Decimal::ONE - bps(p.policy.trailing_stop_bps))
                {
                    Some("Trailing stop")
                } else if now().saturating_sub(p.opened_at) >= p.policy.time_stop_seconds {
                    Some("Time stop")
                } else {
                    None
                };
                reason.map(|r| (p.id.clone(), r))
            })
            .collect();
        for (position, reason) in exits {
            if let Err(error) = self.close_position(&position, reason, "supervisor").await {
                self.event("position.exit_blocked", &error, "supervisor");
            }
        }
        self.event(
            "runtime.tick",
            "Market marks and deterministic exit rules evaluated.",
            "supervisor",
        );
        self.persist(None)
    }
}

fn initial_markets(_mode: TradingMode) -> Vec<Market> {
    Vec::new()
}
fn initial_agents(_mode: TradingMode) -> Vec<AgentRun> {
    [
        (
            "orchestrator",
            "Orchestrator",
            "Plan the cycle from risk policy, positions and trade memory",
        ),
        (
            "market_analyst",
            "Market Analyst",
            "Evaluate live prices and historical trade outcomes",
        ),
        (
            "onchain_analyst",
            "On-chain Analyst",
            "Inspect mainnet mint state, authorities and evidence gaps",
        ),
        (
            "strategy_evaluation",
            "Strategy & Evaluation",
            "Synthesize all reports into open, close or hold",
        ),
    ]
    .into_iter()
    .map(|(id, name, role)| AgentRun {
        id: id.into(),
        name: name.into(),
        role: role.into(),
        status: "Idle".into(),
        output: "Waiting for a cycle using live evidence and verified trade memory.".into(),
        model: "Not configured".into(),
        cycle_id: None,
        completed_at: None,
    })
    .collect()
}
fn initial_state() -> RuntimeState {
    RuntimeState {
        dry_markets: Vec::new(),
        market_error: None,
        market_slot: 0,
        last_quote: None,
        last_decision: None,
        schema_version: 1,
        mode: TradingMode::DryRun,
        paused: true,
        killed: false,
        session: None,
        archived_sessions: vec![],
        live: LiveState {
            pending_decision: None,
            public_key: None,
            wallet: None,
            pending: None,
            pending_position_id: None,
            activation_expires_at: 0,
            readiness_error: None,
            unlocked: false,
        },
        policy: RiskPolicy::default(),
        markets: initial_markets(TradingMode::DryRun),
        positions: vec![],
        orders: vec![],
        proposals: vec![],
        agents: initial_agents(TradingMode::DryRun),
        events: vec![],
        sequence: 0,
        cycle_count: 0,
    }
}
