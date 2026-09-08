import {
  ArrowRight,
  BrainCircuit,
  GitBranch,
  ScanLine,
  Workflow,
} from "lucide-react";
import type { Agent } from "../../lib/contracts";
import { Badge } from "../atoms/badge";
import { Panel } from "../molecules/panel";
const icons = [Workflow, ScanLine, GitBranch, BrainCircuit];
export function AgentField({
  agents,
  onSelect,
}: {
  agents: Agent[];
  onSelect: (agent: Agent) => void;
}) {
  return (
    <Panel
      title="The agent field"
      eyebrow="02 / INTELLIGENCE"
      action={
        <Badge>
          {agents.length} AGENT{agents.length === 1 ? "" : "S"}
        </Badge>
      }
    >
      <div className="agent-intro">
        <p>
          Live market. Trade history.
          <br />
          <strong>One accountable decision.</strong>
        </p>
        <span className="field-cross" aria-hidden="true">
          ✳
        </span>
      </div>
      <div className="agent-grid">
        {agents.map((agent, index) => {
          const Icon = icons[index % icons.length];
          return (
            <button
              key={agent.id}
              className={`agent-node agent-color-${index}`}
              onClick={() => onSelect(agent)}
            >
              <span className="agent-node-top">
                <span className="agent-icon">
                  <Icon size={21} />
                </span>
                <span className="agent-number">0{index + 1}</span>
              </span>
              <strong>{agent.name}</strong>
              <span className="agent-role">{agent.role}</span>
              <span className="agent-status">
                <span
                  className={`status-dot ${agent.status === "Complete" ? "complete" : "idle"}`}
                />
                {agent.status}
                <ArrowRight size={12} />
              </span>
            </button>
          );
        })}
      </div>
      <div className="agent-boundary">
        <ShieldGlyph />
        <span>Analysis has no access to your signer.</span>
      </div>
    </Panel>
  );
}
function ShieldGlyph() {
  return (
    <span aria-hidden="true" className="boundary-icon">
      ⌘
    </span>
  );
}
