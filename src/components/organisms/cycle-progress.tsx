import { useEffect, useState } from "react";
import { Check, CircleAlert, LoaderCircle } from "lucide-react";
import type { Snapshot } from "../../lib/contracts";

export function CycleProgress({
  snapshot,
  pending,
  submittedAt,
}: {
  snapshot: Snapshot;
  pending: boolean;
  submittedAt: number;
}) {
  const [now, setNow] = useState(Date.now);
  const agents = snapshot.agents;
  const running = agents.some((agent) =>
    ["Running", "Waiting"].includes(agent.status),
  );
  const active = running || pending;
  useEffect(() => {
    if (!active) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [active]);
  if (!active && !agents.some((agent) => agent.cycle_id)) return null;
  const latestStart = [...snapshot.events]
    .reverse()
    .find(
      (e) => e.kind === "agent.started" && e.message.startsWith("Orchestrator"),
    );
  const starting =
    pending && (latestStart?.timestamp ?? 0) * 1000 < submittedAt - 1000;
  const start = Math.max(
    (latestStart?.timestamp ?? 0) * 1000,
    pending ? submittedAt : 0,
  );
  const end = [...snapshot.events]
    .reverse()
    .find((e) =>
      ["agent.cycle.completed", "agent.cycle.failed"].includes(e.kind),
    );
  const seconds = Math.max(
    0,
    Math.floor(((active ? now : (end?.timestamp ?? 0) * 1000) - start) / 1000),
  );
  const failed =
    !active && agents.some((a) => ["Failed", "Skipped"].includes(a.status));
  const complete = starting
    ? 0
    : agents.filter((a) => a.status === "Complete").length;
  const stages = [
    { name: "Orchestrate", agents: agents.slice(0, 1) },
    { name: "Parallel analysis", agents: agents.slice(1, 3) },
    { name: "Decision", agents: agents.slice(3, 4) },
  ];
  const current = stages.find((s) =>
    s.agents.some((a) => a.status === "Running"),
  );
  const label = active
    ? current?.name === "Parallel analysis"
      ? "Analyzing market & on-chain evidence"
      : current?.name === "Decision"
        ? "Evaluating the next decision"
        : current
          ? "Planning the cycle"
          : starting
            ? "Connecting to the cycle…"
            : "Finalizing the cycle…"
    : failed
      ? "Cycle stopped — review agent details"
      : snapshot.cycle_schedule.enabled
        ? "Cycle complete · next cycle scheduled"
        : "Cycle complete";
  const Icon = active ? LoaderCircle : failed ? CircleAlert : Check;
  return (
    <section
      className={`cycle-progress ${active ? "is-active" : ""} ${failed ? "is-failed" : ""}`}
      aria-label="Cycle progress"
    >
      <div className="cycle-summary">
        <span className="cycle-glyph">
          <Icon
            size={20}
            className={active ? "spin" : undefined}
            aria-hidden="true"
          />
        </span>
        <div>
          <span className="eyebrow">AGENT LIFECYCLE</span>
          <strong role="status">{label}</strong>
        </div>
        <span className="cycle-elapsed" aria-label="Cycle elapsed time">
          {Math.floor(seconds / 60)}:{String(seconds % 60).padStart(2, "0")}
        </span>
      </div>
      <ol className="cycle-stages">
        {stages.map((stage, index) => {
          const isRunning =
            !starting && stage.agents.some((a) => a.status === "Running");
          const done =
            !starting &&
            stage.agents.length > 0 &&
            stage.agents.every((a) => a.status === "Complete");
          const stopped = stage.agents.some((a) => a.status === "Failed");
          return (
            <li
              key={stage.name}
              className={
                isRunning
                  ? "is-current"
                  : done
                    ? "is-complete"
                    : stopped
                      ? "is-failed"
                      : ""
              }
              aria-current={isRunning ? "step" : undefined}
            >
              <span>
                {isRunning ? (
                  <LoaderCircle size={13} className="spin" />
                ) : done ? (
                  <Check size={13} />
                ) : stopped ? (
                  <CircleAlert size={13} />
                ) : (
                  index + 1
                )}
              </span>
              {stage.name}
            </li>
          );
        })}
      </ol>
      <div className="cycle-completion">
        <progress
          value={complete}
          max={agents.length || 4}
          aria-label="Completed agents"
        />
        <span>
          {complete}/{agents.length} agents complete
        </span>
      </div>
    </section>
  );
}
