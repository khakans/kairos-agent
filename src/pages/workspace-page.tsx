import { useState, type FormEvent } from "react";
import {
  Check,
  Copy,
  Download,
  ExternalLink,
  LockKeyhole,
  RefreshCw,
  ShieldCheck,
  Wallet,
} from "lucide-react";
import type {
  Action,
  Agent,
  Page,
  Policy,
  Position,
  Snapshot,
} from "../lib/contracts";
import { scopeOf } from "../lib/contracts";
import { number, short, time, usd } from "../lib/format";
import { Badge } from "../components/atoms/badge";
import { Button } from "../components/atoms/button";
import { Field } from "../components/molecules/field";
import { EmptyState, Panel } from "../components/molecules/panel";
import { ActivityFeed } from "../components/organisms/activity-feed";
import { AgentField } from "../components/organisms/agent-field";
import { MarketPanel } from "../components/organisms/market-panel";
import { PositionsTable } from "../components/organisms/positions-table";
import type { DialogState } from "../components/organisms/runtime-dialogs";

type Props = {
  page: Page;
  snapshot: Snapshot;
  command: (action: Action) => void;
  busy: boolean;
  onAgent: (agent: Agent) => void;
  onClose: (position: Position) => void;
  onDialog: (dialog: DialogState) => void;
};
export function WorkspacePage({
  page,
  snapshot,
  command,
  busy,
  onAgent,
  onClose,
  onDialog,
}: Props) {
  const scope = scopeOf(snapshot);
  const dry = snapshot.mode === "DRY_RUN";
  const [filter, setFilter] = useState("");
  const [copied, setCopied] = useState(false);
  const positions = snapshot.positions.filter(
    (p) => p.scope_id === scope && p.mode === snapshot.mode,
  );
  if (page === "Agent Field")
    return (
      <div className="page-narrow">
        <AgentField agents={snapshot.agents} onSelect={onAgent} />
        <div className="info-box">
          <ShieldCheck size={20} />
          <div>
            <strong>Live market + trade memory</strong>
            <p>
              Orchestrator plans the cycle. Market Analyst and On-chain Analyst
              independently evaluate live evidence in parallel. Strategy &amp;
              Evaluation combines their reports into open, close, or hold. All
              four use verified trade memory; approval and risk checks remain
              required.
            </p>
          </div>
        </div>
      </div>
    );
  if (page === "Markets")
    return (
      <>
        <MarketPanel markets={snapshot.markets} />
        <Panel title="Market universe">
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>PAIR</th>
                  <th>PRICE</th>
                  <th>SOURCE</th>
                  <th>LAST UPDATED</th>
                </tr>
              </thead>
              <tbody>
                {snapshot.markets.map((m) => (
                  <tr key={m.symbol}>
                    <td>
                      <strong>{m.symbol}/USDC</strong>
                    </td>
                    <td className="mono">{usd(m.price, 4)}</td>
                    <td>{m.source}</td>
                    <td className="mono">{time(m.updated_at)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </Panel>
      </>
    );
  if (page === "Positions" || page === "Portfolio")
    return (
      <>
        <div className="portfolio-banner">
          <Wallet size={24} />
          <div>
            <span className="eyebrow">
              {dry ? "VIRTUAL ACCOUNT" : "MANAGED LIVE ACCOUNT"}
            </span>
            <h2>{usd(snapshot.portfolio.equity)}</h2>
            <p>
              {usd(snapshot.portfolio.cash)} available ·{" "}
              {usd(snapshot.portfolio.exposure)} in positions
            </p>
          </div>
          <Badge tone="purple">
            {dry
              ? (snapshot.session?.name ?? "NO SESSION")
              : short(scope ?? "Not configured")}
          </Badge>
        </div>
        <Panel
          title={
            page === "Portfolio" ? "Position ledger" : "Position management"
          }
          action={<Badge>{positions.length} RECORDS</Badge>}
        >
          <PositionsTable positions={positions} busy={busy} onClose={onClose} />
        </Panel>
      </>
    );
  if (page === "Signals")
    return (
      <Panel
        title="Trade proposals"
        action={
          <Button
            disabled={busy || snapshot.paused || snapshot.killed}
            onClick={() => command({ action: "run_cycle" })}
          >
            Run cycle
          </Button>
        }
      >
        {snapshot.proposals.filter(
          (p) => p.mode === snapshot.mode && p.scope_id === scope,
        ).length === 0 ? (
          <EmptyState
            title="No proposals yet"
            description="Run a cycle to create a candidate. The risk engine checks every proposal before execution."
          />
        ) : (
          <div className="proposal-list">
            {snapshot.proposals
              .filter((p) => p.mode === snapshot.mode && p.scope_id === scope)
              .slice()
              .reverse()
              .map((p) => (
                <article key={p.id}>
                  <div className="proposal-top">
                    <h3>
                      {p.symbol}/USDC <span className="muted">·</span>{" "}
                      {usd(p.notional)}
                    </h3>
                    <Badge
                      tone={p.status === "Approved" ? "positive" : "neutral"}
                    >
                      {p.status}
                    </Badge>
                  </div>
                  <p>{p.thesis}</p>
                  <span className="mono muted">
                    {short(p.id)} · {p.mode} · {p.reason}
                  </span>
                  {p.status === "Pending" && (
                    <div className="form-actions">
                      <Button
                        variant="outline"
                        size="sm"
                        disabled={busy}
                        onClick={() =>
                          command({ action: "reject_proposal", id: p.id })
                        }
                      >
                        Reject
                      </Button>
                      <Button
                        size="sm"
                        disabled={busy}
                        onClick={() =>
                          command({ action: "approve_proposal", id: p.id })
                        }
                      >
                        {dry ? "Approve simulation" : "Approve live trade"}
                      </Button>
                    </div>
                  )}
                </article>
              ))}
          </div>
        )}
      </Panel>
    );
  if (page === "Orders")
    return (
      <Panel
        title={dry ? "Simulated fills" : "Confirmed live fills"}
        action={
          snapshot.live.pending && (
            <Button
              variant="outline"
              disabled={busy}
              onClick={() => command({ action: "reconcile" })}
            >
              Reconcile pending
            </Button>
          )
        }
      >
        {snapshot.orders.filter(
          (o) => o.mode === snapshot.mode && o.scope_id === scope,
        ).length === 0 ? (
          <EmptyState
            title="No fills recorded"
            description="A live submission is not a fill. Orders appear after virtual reconciliation or confirmed on-chain balance reconciliation."
          />
        ) : (
          <div className="table-scroll">
            <table>
              <thead>
                <tr>
                  <th>TIME</th>
                  <th>PAIR</th>
                  <th>SIDE</th>
                  <th>QUANTITY</th>
                  <th>PRICE</th>
                  <th>FEE (USD)</th>
                  <th>SETTLEMENT</th>
                </tr>
              </thead>
              <tbody>
                {snapshot.orders
                  .filter(
                    (o) => o.mode === snapshot.mode && o.scope_id === scope,
                  )
                  .slice()
                  .reverse()
                  .map((o) => (
                    <tr key={o.id}>
                      <td className="mono">{time(o.timestamp)}</td>
                      <td>{o.symbol}/USDC</td>
                      <td>
                        <Badge tone={o.side === "Buy" ? "positive" : "purple"}>
                          {o.side}
                        </Badge>
                      </td>
                      <td className="mono">{number(o.quantity, 6)}</td>
                      <td className="mono">{usd(o.price, 4)}</td>
                      <td className="mono">{usd(o.fee_usdc, 4)}</td>
                      <td>
                        {o.signature ? (
                          <a
                            target="_blank"
                            rel="noreferrer"
                            href={`https://explorer.solana.com/tx/${o.signature}`}
                          >
                            {short(o.signature)} <ExternalLink size={12} />
                          </a>
                        ) : (
                          <Badge>SIMULATED · NO SIGNATURE</Badge>
                        )}
                      </td>
                    </tr>
                  ))}
              </tbody>
            </table>
          </div>
        )}
      </Panel>
    );
  if (page === "Risk")
    return <RiskSettings snapshot={snapshot} command={command} busy={busy} />;
  if (page === "Adapters")
    return (
      <>
        <Panel
          title="Capability readiness"
          action={<Badge>{dry ? "DRY RUN" : "LIVE"}</Badge>}
        >
          <div className="adapter-list">
            {snapshot.readiness.map((r, index) => (
              <div className="adapter-row" key={r.capability}>
                <span className={`adapter-icon agent-color-${index % 4}`}>
                  <ShieldCheck size={22} />
                </span>
                <div>
                  <strong>{r.capability}</strong>
                  <p>{r.detail}</p>
                  <span className="eyebrow">{r.requirement.toUpperCase()}</span>
                </div>
                <Badge
                  tone={
                    r.status === "Ready"
                      ? "positive"
                      : r.status === "Prohibited"
                        ? "purple"
                        : "warning"
                  }
                >
                  {r.status}
                </Badge>
              </div>
            ))}
          </div>
        </Panel>
        <Panel title="Live provider connection">
          <div className="settings-block">
            <p>
              Jupiter V2 and two independent Solana RPC endpoints are configured
              together with the encrypted wallet vault. Routes are restricted to
              USDC / wrapped SOL on Orca Whirlpool.
            </p>
            <Button
              variant="outline"
              disabled={busy}
              onClick={() =>
                snapshot.live.public_key
                  ? command({ action: "refresh_wallet" })
                  : onDialog("configure")
              }
            >
              {snapshot.live.public_key ? (
                <RefreshCw size={15} />
              ) : (
                <LockKeyhole size={15} />
              )}{" "}
              {snapshot.live.public_key
                ? "Test live connections"
                : "Configure live providers"}
            </Button>
          </div>
        </Panel>
      </>
    );
  if (page === "Wallets")
    return (
      <div className="page-narrow">
        <Panel
          title={dry ? "Wallet not required" : "Dedicated live wallet"}
          action={<Wallet size={20} />}
        >
          {!snapshot.live.public_key ? (
            <EmptyState
              title={
                dry
                  ? "Explore first. Connect when ready."
                  : "Give your runtime a dedicated wallet."
              }
              description={
                dry
                  ? "Dry Run uses an isolated virtual account. You can configure a live wallet independently without changing simulated balances."
                  : "Create or import a dedicated automation wallet. Its private key stays inside the encrypted Rust vault."
              }
              action={
                <Button onClick={() => onDialog("configure")}>
                  Configure live wallet
                </Button>
              }
            />
          ) : (
            <div className="wallet-details">
              <Badge tone={snapshot.live.unlocked ? "positive" : "neutral"} dot>
                {snapshot.live.unlocked ? "UNLOCKED" : "LOCKED"}
              </Badge>
              <label>PUBLIC ADDRESS</label>
              <div className="address-box">
                <code>{snapshot.live.public_key}</code>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="Copy wallet address"
                  onClick={() => {
                    void navigator.clipboard
                      .writeText(snapshot.live.public_key!)
                      .then(() => setCopied(true));
                  }}
                >
                  {copied ? <Check size={16} /> : <Copy size={16} />}
                </Button>
              </div>
              <dl className="detail-list">
                <dt>Network fee balance</dt>
                <dd>
                  {number((snapshot.live.wallet?.sol_lamports ?? 0) / 1e9)} SOL
                </dd>
                <dt>Confirmed USDC</dt>
                <dd>{usd((snapshot.live.wallet?.usdc_atoms ?? 0) / 1e6)}</dd>
                <dt>Wrapped SOL</dt>
                <dd>{number((snapshot.live.wallet?.wsol_atoms ?? 0) / 1e9)}</dd>
              </dl>
              <div className="button-row">
                <Button
                  disabled={busy || dry}
                  onClick={() =>
                    snapshot.live.unlocked
                      ? command({ action: "lock_wallet" })
                      : onDialog("unlock")
                  }
                >
                  {snapshot.live.unlocked ? "Lock wallet" : "Unlock wallet"}
                </Button>
                <Button
                  variant="outline"
                  disabled={busy || !snapshot.live.unlocked}
                  onClick={() => command({ action: "refresh_wallet" })}
                >
                  <RefreshCw size={15} />
                  Refresh balances
                </Button>
              </div>
              {dry && (
                <p className="muted">
                  Select Live to unlock. A signer is prohibited in Dry Run.
                </p>
              )}
            </div>
          )}
        </Panel>
        {snapshot.live.pending && (
          <div className="info-box">
            <div>
              <strong>Reconciliation required</strong>
              <p className="mono">{snapshot.live.pending.signature}</p>
              <Button
                disabled={busy}
                onClick={() => command({ action: "reconcile" })}
              >
                Check confirmation
              </Button>
            </div>
          </div>
        )}
      </div>
    );
  if (page === "Audit Log" || page === "Notifications") {
    const events = snapshot.events.filter(
      (e) =>
        e.mode === snapshot.mode &&
        (page !== "Notifications" ||
          /rejected|security|blocked|unknown/.test(e.kind)) &&
        `${e.kind} ${e.message}`.toLowerCase().includes(filter.toLowerCase()),
    );
    return (
      <Panel
        title={
          page === "Audit Log"
            ? "Append-only audit trail"
            : "Runtime notifications"
        }
        action={<Badge>{events.length} SHOWN</Badge>}
      >
        <div className="filter-row">
          <Field
            label="Filter events"
            type="search"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="Search event type or message…"
          />
        </div>
        {events.length ? (
          <ActivityFeed events={events} limit={150} />
        ) : (
          <EmptyState
            title="All clear"
            description="No matching events in this mode. Critical runtime actions appear here automatically."
          />
        )}
        <div className="panel-note">
          Showing up to 150 recent events. Complete audit history is retained in
          SQLite. Execution and decision records are also stored in
          trade-log/YYYY/MM/DD/ using Asia/Jakarta dates.
        </div>
      </Panel>
    );
  }
  return (
    <>
      <Panel title="Runtime settings">
        <div className="settings-block">
          <h3>Two modes. One deterministic core.</h3>
          <p>
            Both modes use live Jupiter prices and quotes. Dry Run simulates
            fills in an isolated account. Live uses a dedicated encrypted
            wallet, Jupiter V2, and independent RPC checks. Entry proposals
            require owner approval. The runtime supervises stop loss, take
            profit, trailing stop and time stop every five seconds.
          </p>
          <div className="button-row">
            <Button
              variant="outline"
              onClick={() => {
                const blob = new Blob(
                  [
                    JSON.stringify(
                      {
                        mode: snapshot.mode,
                        policy: snapshot.policy,
                        session: snapshot.session,
                      },
                      null,
                      2,
                    ),
                  ],
                  { type: "application/json" },
                );
                const url = URL.createObjectURL(blob);
                const a = document.createElement("a");
                a.href = url;
                a.download = "kairos-configuration.json";
                a.click();
                URL.revokeObjectURL(url);
              }}
            >
              <Download size={15} />
              Export non-secret configuration
            </Button>
          </div>
          <h3>Market and AI connection</h3>
          <p>
            On the Rust host, set KAIROS_JUPITER_API_KEY and
            KAIROS_MARKET_RPC_URL for live data. Set KAIROS_AI_URL (full chat
            completions URL), KAIROS_AI_MODEL, and optional KAIROS_AI_API_KEY
            for a remote or local model, then restart the runtime. Connection
            status and the daily journal path appear in Adapters. Secrets stay
            on the host.
          </p>
          <h3>Trade memory</h3>
          <p>
            Every execution and AI decision is journaled under the runtime data
            directory in trade-log/YYYY/MM/DD/*.json (Asia/Jakarta). Each cycle
            reads up to 32 verified outcome records, bounded to 64 KB, across
            Dry Run and Live with explicit mode labels. This is contextual
            learning, not model retraining. Click Run cycle to evaluate the next
            open, close, or hold; approve a proposal to execute.
          </p>
          <h3>Live validation gate</h3>
          <p>
            Before funding or activation, follow the deployment and validation
            checklist in README.md. Production validation, a funded-wallet
            integration test, and an operator approval record are required. A
            passing fixture smoke test alone does not make live execution
            production ready.
          </p>
          <h3>Session lifecycle</h3>
          <p>
            A restart pauses entries and revokes live activation. Stop a Dry Run
            session before resetting it. Reset creates a new session ID and
            keeps the previous ledger and audit trail.
          </p>
          {snapshot.session && (
            <div className="session-settings">
              <dl className="detail-list">
                <dt>Current session</dt>
                <dd>{snapshot.session.name}</dd>
                <dt>Status</dt>
                <dd>{snapshot.session.status}</dd>
                <dt>Parent session</dt>
                <dd>
                  {snapshot.session.parent_session_id
                    ? short(snapshot.session.parent_session_id)
                    : "None"}
                </dd>
              </dl>
              <div className="button-row">
                <Button
                  variant="outline"
                  disabled={
                    busy || !dry || snapshot.session.status === "Stopped"
                  }
                  onClick={() => command({ action: "stop_session" })}
                >
                  Stop session
                </Button>
                <Button
                  variant="outline"
                  disabled={
                    busy || !dry || snapshot.session.status !== "Stopped"
                  }
                  onClick={() => command({ action: "reset_session" })}
                >
                  Reset as new session
                </Button>
              </div>
            </div>
          )}
          <h3>Available integrations</h3>
          <p>
            This build supports Jupiter market data in both modes, an AI
            decision provider with daily trade memory, and restricted on-chain
            execution. Yellowstone, Telegram, Jito and advanced DEX discovery
            are not yet connected. Unconfigured integrations never produce
            fabricated health or analysis results.
          </p>
        </div>
      </Panel>
    </>
  );
}
function RiskSettings({
  snapshot,
  command,
  busy,
}: {
  snapshot: Snapshot;
  command: (action: Action) => void;
  busy: boolean;
}) {
  const p = snapshot.policy;
  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    const policy = { ...p };
    for (const key of Object.keys(p) as (keyof Policy)[]) {
      const value = String(data.get(key));
      if (
        key === "max_trade_usdc" ||
        key === "max_exposure_usdc" ||
        key === "daily_loss_usdc"
      )
        policy[key] = value;
      else policy[key] = Number(value);
    }
    command({ action: "update_policy", policy });
  }
  return (
    <div className="page-narrow">
      <Panel
        title="Deterministic risk policy"
        action={<Badge tone="purple">RUST-ENFORCED</Badge>}
      >
        <form
          className="settings-block"
          key={JSON.stringify(p)}
          onSubmit={submit}
        >
          <p>
            Pause entries before changing limits. Existing positions retain
            their original exit plan. Updating policy revokes live
            authorization.
          </p>
          <div className="form-grid">
            <Field
              label="Maximum trade (USDC)"
              name="max_trade_usdc"
              type="number"
              min="0.01"
              max="10000"
              step="0.01"
              defaultValue={p.max_trade_usdc}
              required
            />
            <Field
              label="Maximum exposure (USDC)"
              name="max_exposure_usdc"
              type="number"
              min="1"
              max="100000"
              step="0.01"
              defaultValue={p.max_exposure_usdc}
              required
            />
            <Field
              label="Realized loss ceiling (USDC)"
              name="daily_loss_usdc"
              type="number"
              min="0.01"
              step="0.01"
              defaultValue={p.daily_loss_usdc}
              required
            />
            <Field
              label="Maximum positions"
              name="max_positions"
              type="number"
              min="1"
              max="20"
              defaultValue={p.max_positions}
              required
            />
            <Field
              label="Maximum slippage (bps)"
              name="max_slippage_bps"
              type="number"
              min="1"
              max="100"
              defaultValue={p.max_slippage_bps}
              required
            />
            <Field
              label="Stop loss (bps)"
              name="stop_loss_bps"
              type="number"
              min="1"
              max="5000"
              defaultValue={p.stop_loss_bps}
              required
            />
            <Field
              label="Take profit (bps)"
              name="take_profit_bps"
              type="number"
              min="1"
              max="10000"
              defaultValue={p.take_profit_bps}
              required
            />
            <Field
              label="Trailing stop (bps)"
              name="trailing_stop_bps"
              type="number"
              min="1"
              max="5000"
              defaultValue={p.trailing_stop_bps}
              required
            />
            <Field
              label="Time stop (seconds)"
              name="time_stop_seconds"
              type="number"
              min="60"
              max="86400"
              defaultValue={p.time_stop_seconds}
              required
            />
          </div>
          <div className="form-actions">
            <span className="muted">
              {snapshot.paused
                ? "Runtime paused · policy editable"
                : "Pause the runtime to edit"}
            </span>
            <Button type="submit" disabled={busy || !snapshot.paused}>
              Save risk policy
            </Button>
          </div>
        </form>
      </Panel>
    </div>
  );
}
