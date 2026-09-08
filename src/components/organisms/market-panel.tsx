import { useState } from "react";
import { ArrowDownRight, ArrowUpRight, ChevronDown } from "lucide-react";
import type { Market } from "../../lib/contracts";
import { usd, number } from "../../lib/format";
import { TokenIcon } from "../atoms/token-icon";
import { Badge } from "../atoms/badge";
import { Panel, EmptyState } from "../molecules/panel";
export function MarketPanel({ markets }: { markets: Market[] }) {
  const [selected, setSelected] = useState("SOL");
  const market = markets.find((m) => m.symbol === selected) ?? markets[0];
  if (!market)
    return (
      <Panel title="Market intelligence" eyebrow="01 / MARKET">
        <EmptyState
          title="Waiting for a live quote"
          description="Configure the live market connection in the Rust host. Both modes use Jupiter prices; the feed refreshes automatically."
        />
      </Panel>
    );
  const prices = market.history.map(Number);
  const min = Math.min(...prices) * 0.998,
    max = Math.max(...prices) * 1.002;
  const span = max - min || 1;
  const points = prices
    .map(
      (price, i) =>
        `${24 + (i / Math.max(prices.length - 1, 1)) * 640},${190 - ((price - min) / span) * 154}`,
    )
    .join(" ");
  return (
    <Panel
      title="Market intelligence"
      eyebrow="01 / MARKET"
      className="market-panel"
      action={
        <Badge dot tone="purple">
          {Date.now() / 1000 - market.updated_at > 30
            ? "STALE FEED"
            : "LIVE MARKET"}
        </Badge>
      }
    >
      <div className="market-summary">
        <div className="market-pair">
          <TokenIcon symbol={market.symbol} />
          <div>
            <label className="sr-only" htmlFor="market-select">
              Market pair
            </label>
            <div className="select-wrap">
              <select
                id="market-select"
                value={market.symbol}
                onChange={(e) => setSelected(e.target.value)}
              >
                {markets.map((m) => (
                  <option key={m.symbol} value={m.symbol}>
                    {m.symbol} / USDC
                  </option>
                ))}
              </select>
              <ChevronDown size={14} />
            </div>
            <span>
              {market.name} <span className="dot-separator">·</span> Spot market
            </span>
          </div>
        </div>
        <div className="market-price">
          <strong>{usd(market.price, Number(market.price) < 1 ? 4 : 2)}</strong>
          <span
            className={
              market.change_bps >= 0 ? "positive-text" : "negative-text"
            }
          >
            {market.change_bps >= 0 ? (
              <ArrowUpRight size={13} />
            ) : (
              <ArrowDownRight size={13} />
            )}{" "}
            {(market.change_bps / 100).toFixed(2)}%{" "}
            <span className="muted">last step</span>
          </span>
        </div>
      </div>
      <div className="chart-toolbar">
        <span className="chart-label">PRICE / USDC</span>
        <span className="chart-window">{prices.length} OBSERVATIONS</span>
      </div>
      <div className="chart-container">
        <svg
          viewBox="0 0 750 224"
          role="img"
          aria-label={`${market.symbol} price history from ${market.source}`}
          preserveAspectRatio="none"
        >
          {[0, 1, 2, 3].map((i) => (
            <g key={i}>
              <line
                x1="24"
                y1={36 + i * 51}
                x2="664"
                y2={36 + i * 51}
                stroke="#e9e9e4"
                strokeDasharray="3 5"
              />
              <text x="684" y={40 + i * 51} className="chart-axis">
                {number(max - (i * span) / 3, 2)}
              </text>
            </g>
          ))}
          <polygon
            points={`24,205 ${points} 664,205`}
            fill="#eef3ea"
            opacity="0.8"
          />
          <polyline
            points={points}
            fill="none"
            stroke="#627b51"
            strokeWidth="2.4"
            strokeLinejoin="round"
            strokeLinecap="round"
          />
          <circle
            cx="664"
            cy={190 - ((prices[prices.length - 1] - min) / span) * 154}
            r="4"
            fill="#627b51"
            stroke="white"
            strokeWidth="2"
          />
          <text x="24" y="222" className="chart-axis">
            EARLIER
          </text>
          <text x="626" y="222" className="chart-axis">
            LATEST
          </text>
        </svg>
      </div>
      <div className="market-footnote">
        <span>
          <span className="status-dot" /> {market.source}
        </span>
        <span>
          {"Updated " + new Date(market.updated_at * 1000).toLocaleTimeString()}
        </span>
      </div>
    </Panel>
  );
}
