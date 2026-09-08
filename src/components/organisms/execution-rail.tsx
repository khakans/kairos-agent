import { ArrowRight, Check, LockKeyhole } from "lucide-react";
import type { Snapshot } from "../../lib/contracts";
import { scopeOf } from "../../lib/contracts";
export function ExecutionRail({ snapshot }: { snapshot: Snapshot }) {
  const dry = snapshot.mode === "DRY_RUN";
  const hasFill = snapshot.orders.some(
    (o) => o.mode === snapshot.mode && o.scope_id === scopeOf(snapshot),
  );
  const steps = dry
    ? ["Discover", "Analyze", "Risk review", "Simulate", "Virtual fill"]
    : ["Propose", "Risk review", "Simulate", "Sign & submit", "Reconcile"];
  return (
    <section
      className="execution-rail"
      aria-label="Deterministic execution pipeline"
    >
      <div className="rail-title">
        <span className="eyebrow">EXECUTION RAIL</span>
        <strong>Trust is a sequence.</strong>
      </div>
      <ol>
        {steps.map((step, index) => (
          <li key={step}>
            <span className={`rail-step ${hasFill ? "rail-complete" : ""}`}>
              {hasFill ? <Check size={12} /> : index + 1}
            </span>
            <span>{step}</span>
            {index < steps.length - 1 && (
              <ArrowRight className="rail-arrow" size={13} />
            )}
          </li>
        ))}
      </ol>
      <div className="rail-lock">
        <LockKeyhole size={15} />
        <span>{dry ? "SIGNER BLOCKED" : "INTENT-BOUND SIGNER"}</span>
      </div>
    </section>
  );
}
