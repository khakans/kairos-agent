use kairos_domain::{RiskPolicy, TradingMode};
use kairos_live::{PendingTrade, WalletSnapshot};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Market {
    pub symbol: String,
    pub name: String,
    pub price: Decimal,
    pub change_bps: i32,
    pub history: Vec<Decimal>,
    pub source: String,
    pub updated_at: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub scope_id: String,
    pub mode: TradingMode,
    pub symbol: String,
    pub quantity: Decimal,
    pub entry_price: Decimal,
    pub mark_price: Decimal,
    pub high_price: Decimal,
    pub cost_basis: Decimal,
    pub realized_pnl: Decimal,
    pub status: String,
    pub opened_at: u64,
    pub exit_reason: Option<String>,
    pub policy: RiskPolicy,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Order {
    #[serde(default)]
    pub position_id: Option<String>,
    #[serde(default)]
    pub decision_id: Option<String>,
    pub id: String,
    pub scope_id: String,
    pub mode: TradingMode,
    pub symbol: String,
    pub side: String,
    pub quantity: Decimal,
    pub price: Decimal,
    pub fee_usdc: Decimal,
    pub fee_lamports: u64,
    pub signature: Option<String>,
    pub timestamp: u64,
    pub reason: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Proposal {
    #[serde(default)]
    pub decision_evidence: Option<serde_json::Value>,
    #[serde(default)]
    pub position_id: Option<String>,
    #[serde(default)]
    pub decision_id: Option<String>,
    pub id: String,
    pub scope_id: String,
    pub mode: TradingMode,
    pub symbol: String,
    pub notional: Decimal,
    pub thesis: String,
    pub status: String,
    pub reason: String,
    pub created_at: u64,
    pub expires_at: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct AgentRun {
    pub id: String,
    pub name: String,
    pub role: String,
    pub status: String,
    pub output: String,
    pub model: String,
    pub cycle_id: Option<String>,
    pub completed_at: Option<u64>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: String,
    pub sequence: u64,
    pub mode: TradingMode,
    pub scope_id: Option<String>,
    pub timestamp: u64,
    pub kind: String,
    pub message: String,
    pub correlation_id: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct DrySession {
    pub id: String,
    pub parent_session_id: Option<String>,
    pub name: String,
    pub status: String,
    pub starting_balance: Decimal,
    pub balance: Decimal,
    pub fee_bps: u16,
    pub slippage_bps: u16,
    pub created_at: u64,
    pub policy: RiskPolicy,
    pub source: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct LiveState {
    #[serde(default)]
    pub pending_decision: Option<serde_json::Value>,
    pub public_key: Option<String>,
    pub wallet: Option<WalletSnapshot>,
    pub pending: Option<PendingTrade>,
    pub pending_position_id: Option<String>,
    pub activation_expires_at: u64,
    pub readiness_error: Option<String>,
    pub unlocked: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct RuntimeState {
    #[serde(default)]
    pub market_error: Option<String>,
    #[serde(default)]
    pub market_slot: u64,
    #[serde(default)]
    pub last_quote: Option<kairos_live::LiveQuote>,
    #[serde(default)]
    pub last_decision: Option<serde_json::Value>,
    #[serde(default)]
    pub dry_markets: Vec<Market>,
    pub schema_version: u32,
    pub mode: TradingMode,
    pub paused: bool,
    pub killed: bool,
    pub session: Option<DrySession>,
    pub archived_sessions: Vec<DrySession>,
    pub live: LiveState,
    pub policy: RiskPolicy,
    pub markets: Vec<Market>,
    pub positions: Vec<Position>,
    pub orders: Vec<Order>,
    pub proposals: Vec<Proposal>,
    pub agents: Vec<AgentRun>,
    pub events: Vec<AuditEvent>,
    pub sequence: u64,
    pub cycle_count: u64,
}
#[derive(Clone, Serialize)]
pub struct Portfolio {
    pub equity: Decimal,
    pub cash: Decimal,
    pub exposure: Decimal,
    pub realized_pnl: Decimal,
    pub unrealized_pnl: Decimal,
    pub fees: Decimal,
    pub open_positions: usize,
}
#[derive(Serialize)]
pub struct Readiness {
    pub capability: String,
    pub requirement: String,
    pub status: String,
    pub detail: String,
}
#[derive(Serialize)]
pub struct Snapshot {
    #[serde(flatten)]
    pub state: RuntimeState,
    pub portfolio: Portfolio,
    pub readiness: Vec<Readiness>,
    pub capabilities: kairos_domain::Capabilities,
}

#[derive(Deserialize)]
pub struct Command {
    pub request_id: String,
    #[serde(flatten)]
    pub action: Action,
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    CreateSession {
        name: String,
        starting_balance: Decimal,
        fee_bps: u16,
        slippage_bps: u16,
    },
    StartSession,
    Pause,
    Resume,
    StopSession,
    ResetSession,
    Kill,
    SetMode {
        mode: TradingMode,
    },
    RunCycle,
    ApproveProposal {
        id: String,
    },
    RejectProposal {
        id: String,
    },
    ClosePosition {
        id: String,
    },
    UpdatePolicy {
        policy: RiskPolicy,
    },
    ConfigureLive {
        rpc_url: String,
        secondary_rpc_url: String,
        jupiter_api_key: String,
        keypair_json: Option<String>,
        passphrase: String,
    },
    UnlockWallet {
        passphrase: String,
    },
    LockWallet,
    RefreshWallet,
    ActivateLive {
        confirmation: String,
    },
    Reconcile,
}
