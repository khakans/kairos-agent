import { useEffect, useState } from "react";
import { Clock3, Play } from "lucide-react";
import type { Action, Snapshot } from "../../lib/contracts";
import { Button } from "../atoms/button";

export function CycleControls({
  snapshot,
  busy,
  command,
}: {
  snapshot: Snapshot;
  busy: boolean;
  command: (action: Action) => void;
}) {
  const schedule = snapshot.cycle_schedule;
  const [unit, setUnit] = useState(60);
  const [amount, setAmount] = useState("5");
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const scale = schedule.interval_seconds % 3600 === 0 ? 3600 : 60;
    setUnit(scale);
    setAmount(String(schedule.interval_seconds / scale));
  }, [schedule.interval_seconds]);
  useEffect(() => {
    if (!schedule.enabled) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [schedule.enabled]);
  const seconds = Math.max(
    0,
    Math.ceil((schedule.next_run_at ?? 0) - now / 1000),
  );
  const duration = Number(amount) * unit;
  const valid =
    Number.isInteger(Number(amount)) && duration >= 60 && duration <= 604800;
  const active = snapshot.agents.some((a) =>
    ["Running", "Waiting"].includes(a.status),
  );
  return (
    <section className="cycle-controls" aria-label="Automatic cycle settings">
      <div className="cycle-schedule-status">
        <Clock3 size={18} aria-hidden="true" />
        <div>
          <strong>
            {schedule.enabled
              ? active
                ? "Automatic cycles running"
                : seconds > 0
                  ? "Waiting for next cycle"
                  : "Next cycle starting"
              : "Automatic cycles stopped"}
          </strong>
          <span>
            {schedule.enabled && schedule.next_run_at
              ? `Next cycle: ${new Date(schedule.next_run_at * 1000).toLocaleTimeString()} · ${Math.floor(seconds / 3600)}:${String(Math.floor(seconds / 60) % 60).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")} remaining`
              : "The interval starts after each cycle finishes."}
          </span>
          {schedule.last_error && (
            <span className="form-error">
              Last cycle: {schedule.last_error}
            </span>
          )}
        </div>
      </div>
      <form
        className="cycle-interval"
        onSubmit={(event) => {
          event.preventDefault();
          if (valid)
            command({ action: "configure_cycles", interval_seconds: duration });
        }}
      >
        <label>
          Interval
          <input
            aria-label="Cycle interval"
            type="number"
            min="1"
            max={604800 / unit}
            step="1"
            value={amount}
            onChange={(event) => setAmount(event.target.value)}
            required
          />
        </label>
        <label>
          Unit
          <select
            aria-label="Interval unit"
            value={unit}
            onChange={(event) => setUnit(Number(event.target.value))}
          >
            <option value={60}>Minutes</option>
            <option value={3600}>Hours</option>
          </select>
        </label>
        <Button
          size="sm"
          variant="outline"
          type="submit"
          disabled={busy || !valid || duration === schedule.interval_seconds}
        >
          Save interval
        </Button>
        <Button
          size="sm"
          variant="ghost"
          type="button"
          disabled={
            busy || schedule.enabled || snapshot.paused || snapshot.killed
          }
          onClick={() => command({ action: "run_cycle" })}
        >
          <Play size={12} />
          Run once
        </Button>
        <small>
          Saved:{" "}
          {schedule.interval_seconds % 3600 === 0
            ? `${schedule.interval_seconds / 3600} hour(s)`
            : `${schedule.interval_seconds / 60} minute(s)`}
          .{" "}
          {schedule.enabled
            ? "Saving restarts the waiting period."
            : "Start cycles uses this saved interval."}
        </small>
      </form>
    </section>
  );
}
