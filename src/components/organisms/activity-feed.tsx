import { ArrowUpRight, CircleCheck, ShieldAlert } from "lucide-react";
import type { Snapshot } from "../../lib/contracts";
import { time } from "../../lib/format";
export function ActivityFeed({
  events,
  limit = 5,
}: {
  events: Snapshot["events"];
  limit?: number;
}) {
  return (
    <div className="activity-feed">
      {events
        .slice(-limit)
        .reverse()
        .map((event) => (
          <div className="activity-item" key={event.id}>
            <span
              className={`activity-icon ${event.kind.includes("rejected") || event.kind.includes("security") ? "activity-warning" : ""}`}
            >
              {event.kind.includes("rejected") ? (
                <ShieldAlert size={15} />
              ) : event.kind.includes("filled") ? (
                <CircleCheck size={15} />
              ) : (
                <ArrowUpRight size={15} />
              )}
            </span>
            <div>
              <div className="activity-heading">
                <strong>{event.kind.replaceAll("_", " ")}</strong>
                <time>{time(event.timestamp)}</time>
              </div>
              <p>{event.message}</p>
              <span className="activity-scope">
                {event.mode} <span>·</span> EVENT{" "}
                {String(event.sequence).padStart(4, "0")}
              </span>
            </div>
          </div>
        ))}
    </div>
  );
}
