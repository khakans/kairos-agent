import type { FormEvent } from "react";
import { Radio } from "lucide-react";
import type { Action, Snapshot } from "../../lib/contracts";
import { number, short, time, usd } from "../../lib/format";
import { Badge } from "../atoms/badge";
import { Button } from "../atoms/button";
import { Field } from "../molecules/field";
import { EmptyState, Panel } from "../molecules/panel";

export function PumpfunDiscovery({
  snapshot,
  command,
  busy,
}: {
  snapshot: Snapshot;
  command: (action: Action) => void;
  busy: boolean;
}) {
  const { pumpfun: feed, pumpfun_config: config } = snapshot;
  function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    command({
      action: "configure_pumpfun",
      config: {
        enabled: config.enabled,
        max_pair_age_hours: Number(data.get("max_pair_age_hours")),
        min_liquidity_usd: String(data.get("min_liquidity_usd")),
        min_volume_h1_usd: String(data.get("min_volume_h1_usd")),
        min_abs_change_h1_pct: String(data.get("min_abs_change_h1_pct")),
      },
    });
  }
  return (
    <Panel
      title="Pump.fun discovery"
      action={
        <Badge
          tone={
            feed.connected ? "positive" : config.enabled ? "warning" : "neutral"
          }
          dot
        >
          {feed.connected
            ? "LIVE FEED"
            : config.enabled
              ? "CONNECTING"
              : "DISABLED"}
        </Badge>
      }
    >
      <div className="pumpfun-intro">
        <div>
          <strong>New pairs · volume · price movement</strong>
          <p>
            PumpPortal discoveries enriched with DEX Screener metrics for all
            four agents. Discovery and analysis only; memecoin execution is not
            enabled.
          </p>
        </div>
        <Button
          variant={config.enabled ? "outline" : "default"}
          disabled={busy}
          onClick={() =>
            command({
              action: "configure_pumpfun",
              config: { ...config, enabled: !config.enabled },
            })
          }
        >
          <Radio size={15} />
          {config.enabled ? "Disable discovery" : "Enable discovery"}
        </Button>
      </div>
      <form
        key={JSON.stringify(config)}
        onSubmit={save}
        className="pumpfun-filters"
      >
        <div className="form-grid">
          <Field
            label="Maximum pair age (hours)"
            name="max_pair_age_hours"
            type="number"
            min="1"
            max="168"
            step="1"
            required
            defaultValue={config.max_pair_age_hours}
          />
          <Field
            label="Minimum liquidity (USD)"
            name="min_liquidity_usd"
            type="number"
            min="0"
            max="1000000000"
            step="0.01"
            required
            defaultValue={config.min_liquidity_usd}
          />
          <Field
            label="Minimum 1h volume (USD)"
            name="min_volume_h1_usd"
            type="number"
            min="0"
            max="1000000000"
            step="0.01"
            required
            defaultValue={config.min_volume_h1_usd}
          />
          <Field
            label="Minimum absolute 1h change (%)"
            name="min_abs_change_h1_pct"
            type="number"
            min="0"
            max="10000"
            step="0.01"
            required
            defaultValue={config.min_abs_change_h1_pct}
            hint="Price movement proxy, not statistical volatility."
          />
        </div>
        <div className="form-actions">
          <span className="muted">
            Filters saved in Rust storage; both modes use the same live
            discovery feed.
          </span>
          <Button type="submit" variant="outline" disabled={busy}>
            Save discovery filters
          </Button>
        </div>
      </form>
      {feed.error && (
        <p role="status" className="pumpfun-error">
          {feed.error}
        </p>
      )}
      <div className="pumpfun-summary muted">
        <span>
          {feed.tracked} tracked · {feed.matching} displayed matches
        </span>
        <span>
          Metrics:{" "}
          {feed.last_refresh_at ? time(feed.last_refresh_at) : "awaiting data"}
        </span>
      </div>
      {feed.candidates.length === 0 ? (
        <EmptyState
          title={
            config.enabled
              ? "Listening for new Pump.fun tokens"
              : "Discovery is disabled"
          }
          description={
            config.enabled
              ? "New token and migration events appear here. Pair metrics require DEX Screener indexing; missing data never passes the filters."
              : "Enable the adapter to discover live token creation and migration events."
          }
        />
      ) : (
        <div className="table-scroll">
          <table className="pumpfun-table">
            <thead>
              <tr>
                <th>TOKEN / MINT</th>
                <th>PAIR AGE</th>
                <th>PRICE</th>
                <th>LIQUIDITY</th>
                <th>1H VOLUME</th>
                <th>1H CHANGE</th>
                <th>SCREENING</th>
              </tr>
            </thead>
            <tbody>
              {feed.candidates.map((candidate) => (
                <tr key={candidate.mint}>
                  <td>
                    <strong>{candidate.symbol || "Unnamed token"}</strong>
                    <span className="pumpfun-detail">
                      {candidate.name || candidate.event}
                    </span>
                    <a
                      href={`https://pump.fun/coin/${candidate.mint}`}
                      target="_blank"
                      rel="noreferrer"
                      title={candidate.mint}
                      className="mono"
                    >
                      {short(candidate.mint)}
                    </a>
                  </td>
                  <td>
                    {candidate.pair
                      ? `${number(Math.max(0, candidate.pair.checked_at - candidate.pair.created_at) / 3600, 1)}h`
                      : "Unknown"}
                    <span className="pumpfun-detail">
                      {candidate.event === "migration"
                        ? "Migration observed"
                        : "Creation observed"}
                    </span>
                  </td>
                  <td className="mono">
                    {candidate.pair ? usd(candidate.pair.price_usd, 10) : "—"}
                  </td>
                  <td className="mono">
                    {candidate.pair?.liquidity_usd != null
                      ? usd(candidate.pair.liquidity_usd, 0)
                      : "Unknown"}
                  </td>
                  <td className="mono">
                    {candidate.pair
                      ? usd(candidate.pair.volume_h1_usd, 0)
                      : "—"}
                  </td>
                  <td className="mono">
                    {candidate.pair
                      ? `${number(candidate.pair.change_h1_pct, 2)}%`
                      : "—"}
                  </td>
                  <td>
                    <Badge
                      tone={candidate.matches_filters ? "positive" : "warning"}
                    >
                      {candidate.matches_filters
                        ? "MATCHES FILTERS"
                        : "EXCLUDED"}
                    </Badge>
                    <span className="pumpfun-detail">
                      {candidate.matches_filters
                        ? "For AI analysis · security not verified"
                        : candidate.reasons.join(" · ")}
                    </span>
                    {candidate.pair && (
                      <span className="pumpfun-detail">
                        {candidate.pair.dex} · {time(candidate.pair.checked_at)}
                      </span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <p className="pumpfun-footnote muted">
        Rolling window of 300 observed tokens, showing up to 30; up to 5
        matching candidates enter each agent cycle. No historical backfill.
        Metrics older than 90 seconds and Mayhem-mode tokens are excluded.
        Matching filters does not verify token security or authentic trading
        volume.
      </p>
    </Panel>
  );
}
