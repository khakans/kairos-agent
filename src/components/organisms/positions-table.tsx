import type { Position } from "../../lib/contracts";
import { number, usd } from "../../lib/format";
import { Badge } from "../atoms/badge";
import { Button } from "../atoms/button";
import { TokenIcon } from "../atoms/token-icon";
import { EmptyState } from "../molecules/panel";
export function PositionsTable({
  positions,
  busy,
  onClose,
}: {
  positions: Position[];
  busy: boolean;
  onClose: (position: Position) => void;
}) {
  if (!positions.length)
    return (
      <EmptyState
        title="Room for your next decision"
        description="Run a cycle and approve a proposal. Positions appear here only after a simulated fill or confirmed live reconciliation."
      />
    );
  return (
    <div className="table-scroll">
      <table>
        <thead>
          <tr>
            <th>ASSET / STRATEGY</th>
            <th>QUANTITY</th>
            <th>ENTRY PRICE</th>
            <th>MARK PRICE</th>
            <th>STATUS</th>
            <th>CONTROL</th>
          </tr>
        </thead>
        <tbody>
          {positions.map((p) => (
            <tr key={p.id}>
              <td>
                <div className="asset-cell">
                  <TokenIcon symbol={p.symbol} />
                  <div>
                    <strong>{p.symbol} / USDC</strong>
                    <span>
                      {p.mode === "DRY_RUN"
                        ? "Simulated · live market"
                        : "Live · manual v1"}
                    </span>
                  </div>
                </div>
              </td>
              <td className="mono">{number(p.quantity, 6)}</td>
              <td className="mono">{usd(p.entry_price, 4)}</td>
              <td className="mono">{usd(p.mark_price, 4)}</td>
              <td>
                <Badge tone={p.status === "Open" ? "positive" : "neutral"} dot>
                  {p.status}
                </Badge>
              </td>
              <td>
                {p.status === "Open" ? (
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={busy}
                    onClick={() => onClose(p)}
                  >
                    Close position
                  </Button>
                ) : (
                  <span className="muted">
                    {p.exit_reason ?? "Awaiting confirmation"}
                  </span>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
