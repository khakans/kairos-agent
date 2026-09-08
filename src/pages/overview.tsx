import {
  ArrowDownToLine,
  ArrowRight,
  ChartNoAxesCombined,
  Layers3,
  ShieldCheck,
  Wallet,
} from "lucide-react";
import type { Action, Agent, Page, Position, Snapshot } from "../lib/contracts";
import { scopeOf } from "../lib/contracts";
import { usd } from "../lib/format";
import { Badge } from "../components/atoms/badge";
import { Button } from "../components/atoms/button";
import { Panel } from "../components/molecules/panel";
import { StatCard } from "../components/molecules/stat-card";
import { ActivityFeed } from "../components/organisms/activity-feed";
import { AgentField } from "../components/organisms/agent-field";
import { ExecutionRail } from "../components/organisms/execution-rail";
import { MarketPanel } from "../components/organisms/market-panel";
import { PositionsTable } from "../components/organisms/positions-table";

export function Overview({
  snapshot,
  onNavigate,
  onAgent,
  onClose,
  command,
  busy,
}: {
  snapshot: Snapshot;
  onNavigate: (page: Page) => void;
  onAgent: (agent: Agent) => void;
  onClose: (position: Position) => void;
  command: (action: Action) => void;
  busy: boolean;
}) {
  const dry = snapshot.mode === "DRY_RUN";
  const p = snapshot.portfolio;
  const scope = scopeOf(snapshot);
  const positions = snapshot.positions.filter(
    (p) =>
      p.mode === snapshot.mode && p.scope_id === scope && p.status !== "Closed",
  );
  const proposal = snapshot.proposals
    .filter((p) => p.mode === snapshot.mode && p.scope_id === scope)
    .at(-1);
  const exposure = Math.min(
    100,
    (Number(p.exposure) / Number(snapshot.policy.max_exposure_usdc)) * 100,
  );
  return (
    <>
      <div className="metrics-grid">
        <StatCard
          label={dry ? "VIRTUAL EQUITY" : "MANAGED LIVE EQUITY"}
          value={usd(p.equity)}
          detail={
            dry
              ? "Isolated from on-chain balances"
              : "Cash + managed positions; external SOL excluded"
          }
          icon={Wallet}
          accent
        />
        <StatCard
          label={dry ? "VIRTUAL REALIZED P&L" : "REALIZED P&L"}
          value={usd(p.realized_pnl)}
          detail={`${usd(p.unrealized_pnl)} unrealized · current account`}
          icon={ChartNoAxesCombined}
        />
        <StatCard
          label="OPEN EXPOSURE"
          value={usd(p.exposure)}
          detail={`of ${usd(snapshot.policy.max_exposure_usdc)} configured limit`}
          icon={Layers3}
        />
        <StatCard
          label="OPEN POSITIONS"
          value={`${p.open_positions.toString().padStart(2, "0")} / ${snapshot.policy.max_positions.toString().padStart(2, "0")}`}
          detail="Each position has its own exit plan"
          icon={ShieldCheck}
        />
      </div>
      <div className="intelligence-grid">
        <MarketPanel markets={snapshot.markets} />
        <AgentField agents={snapshot.agents} onSelect={onAgent} />
      </div>
      <ExecutionRail snapshot={snapshot} />
      <div className="decision-grid">
        <Panel
          title="Latest decision"
          eyebrow="03 / STRATEGY"
          action={
            <Badge
              tone={proposal?.status === "Approved" ? "positive" : "neutral"}
            >
              {proposal?.status.toUpperCase() ?? "AWAITING CYCLE"}
            </Badge>
          }
        >
          <div className="decision-content">
            <div className="decision-identity">
              <span className="decision-glyph">
                <ArrowDownToLine size={22} />
              </span>
              <div>
                <h3>
                  {proposal
                    ? `${proposal.symbol} / USDC`
                    : "Your next signal starts here."}
                </h3>
                <span>
                  {proposal
                    ? `${dry ? "SIMULATED" : "LIVE"} ${proposal.position_id ? "CLOSE POSITION" : "BUY"} · ${usd(proposal.notional)}`
                    : "RUN A CYCLE TO CREATE A PROPOSAL"}
                </span>
              </div>
            </div>
            <p>
              {proposal?.thesis ??
                "Every proposal carries its evidence and passes through deterministic risk checks. Start a session to inspect the complete workflow."}
            </p>
            {proposal?.status === "Pending" ? (
              <div className="decision-actions">
                <Button
                  size="sm"
                  disabled={busy}
                  onClick={() =>
                    command({ action: "approve_proposal", id: proposal.id })
                  }
                >
                  {dry ? "Approve simulation" : "Approve live trade"}
                  <ArrowRight size={14} />
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={busy}
                  onClick={() =>
                    command({ action: "reject_proposal", id: proposal.id })
                  }
                >
                  Reject
                </Button>
                <span className="muted">Expires after 60s</span>
              </div>
            ) : (
              <div className="decision-actions">
                <span className="muted">
                  {proposal?.reason ??
                    "AI proposes. Deterministic risk has the final say."}
                </span>
              </div>
            )}
          </div>
        </Panel>
        <Panel
          title="Capital guardrails"
          eyebrow="04 / RISK"
          action={
            <Button
              variant="ghost"
              size="sm"
              onClick={() => onNavigate("Risk")}
            >
              Policy
              <ArrowRight size={13} />
            </Button>
          }
        >
          <div className="risk-content">
            <div className="risk-meter-label">
              <span>Portfolio exposure</span>
              <strong>{exposure.toFixed(1)}%</strong>
            </div>
            <progress
              value={exposure}
              max="100"
              aria-label="Portfolio exposure usage"
            />
            <div className="risk-rows">
              <div>
                <span>Maximum per trade</span>
                <strong>{usd(snapshot.policy.max_trade_usdc)}</strong>
              </div>
              <div>
                <span>Realized loss ceiling</span>
                <strong>{usd(snapshot.policy.daily_loss_usdc)}</strong>
              </div>
              <div>
                <span>Maximum slippage</span>
                <strong>{snapshot.policy.max_slippage_bps} bps</strong>
              </div>
              <div>
                <span>Signing capability</span>
                <Badge tone={dry ? "purple" : "neutral"}>
                  {dry
                    ? "PROHIBITED"
                    : snapshot.capabilities.signing_enabled
                      ? "AUTHORIZED"
                      : "LOCKED"}
                </Badge>
              </div>
            </div>
          </div>
        </Panel>
      </div>
      <Panel
        title={dry ? "Simulated positions" : "Live positions"}
        eyebrow="05 / PORTFOLIO"
        action={
          <Button
            variant="ghost"
            size="sm"
            onClick={() => onNavigate("Positions")}
          >
            All positions
            <ArrowRight size={13} />
          </Button>
        }
      >
        <PositionsTable
          positions={positions.slice(0, 4)}
          busy={busy}
          onClose={onClose}
        />
      </Panel>
      <div className="bottom-grid">
        <Panel
          title="Runtime activity"
          action={
            <Button
              variant="ghost"
              size="sm"
              onClick={() => onNavigate("Audit Log")}
            >
              Open audit log
              <ArrowRight size={13} />
            </Button>
          }
        >
          <ActivityFeed
            events={snapshot.events.filter((e) => e.mode === snapshot.mode)}
          />
        </Panel>
        <Panel
          title="Connection readiness"
          action={
            <Badge>
              {snapshot.readiness.filter((r) => r.status === "Ready").length}{" "}
              READY
            </Badge>
          }
        >
          <div className="readiness-list">
            {snapshot.readiness.map((item) => (
              <div key={item.capability}>
                <span>
                  <strong>{item.capability}</strong>
                  <small>{item.requirement}</small>
                </span>
                <Badge
                  tone={
                    item.status === "Ready"
                      ? "positive"
                      : item.status === "Prohibited"
                        ? "purple"
                        : "neutral"
                  }
                  dot
                >
                  {item.status}
                </Badge>
              </div>
            ))}
          </div>
          <button
            className="panel-bottom-link"
            onClick={() => onNavigate("Adapters")}
          >
            Inspect capabilities
            <ArrowRight size={15} />
          </button>
        </Panel>
      </div>
    </>
  );
}
