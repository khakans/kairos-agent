import {
  Activity,
  ArrowUpRight,
  Bell,
  Boxes,
  CircleHelp,
  ClipboardList,
  Command,
  FileClock,
  Globe,
  Layers3,
  LayoutDashboard,
  Menu,
  Radio,
  Settings2,
  ShieldCheck,
  Wallet,
  X,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { useState, type ReactNode } from "react";
import type { Mode, Page } from "../../lib/contracts";
import { Badge } from "../atoms/badge";
import { Button } from "../atoms/button";
const primary: [Page, LucideIcon][] = [
  ["Overview", LayoutDashboard],
  ["Agent Field", Boxes],
  ["Markets", Globe],
  ["Signals", Radio],
  ["Portfolio", Layers3],
  ["Positions", Activity],
  ["Orders", ClipboardList],
];
const operations: [Page, LucideIcon][] = [
  ["Risk", ShieldCheck],
  ["Adapters", Zap],
  ["Wallets", Wallet],
  ["Notifications", Bell],
  ["Audit Log", FileClock],
  ["Settings", Settings2],
];
export function CommandLayout({
  page,
  onNavigate,
  mode,
  onMode,
  busy,
  children,
  actions,
}: {
  page: Page;
  onNavigate: (page: Page) => void;
  mode: Mode;
  onMode: (mode: Mode) => void;
  busy: boolean;
  children: ReactNode;
  actions: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  function navigate(page: Page) {
    onNavigate(page);
    setOpen(false);
  }
  return (
    <div className="app-shell">
      <a href="#main-content" className="skip-link">
        Skip to content
      </a>
      {open && (
        <button
          className="nav-scrim"
          aria-label="Close navigation"
          onClick={() => setOpen(false)}
        />
      )}
      <aside className={`sidebar ${open ? "is-open" : ""}`}>
        <div className="brand">
          <span className="brand-mark">
            <Command size={22} />
          </span>
          <div>
            kairos<span className="brand-agent">agent</span>
          </div>
          <span className="brand-version">/ 01</span>
          <Button
            className="mobile-only"
            variant="ghost"
            size="icon"
            aria-label="Close menu"
            onClick={() => setOpen(false)}
          >
            <X size={18} />
          </Button>
        </div>
        <div className="workspace-switch">
          <span className="workspace-avatar">K</span>
          <div>
            <strong>My workspace</strong>
            <span>Personal runtime</span>
          </div>
          <Badge>OWNER</Badge>
        </div>
        <nav aria-label="Main navigation">
          <p className="nav-label">WORKSPACE</p>
          {primary.map(([name, Icon]) => (
            <button
              key={name}
              aria-label={name}
              onClick={() => navigate(name)}
              aria-current={page === name ? "page" : undefined}
              className={`nav-item ${page === name ? "active" : ""}`}
            >
              <Icon size={17} />
              {name}
            </button>
          ))}
          <p className="nav-label operations-label">OPERATIONS</p>
          {operations.map(([name, Icon]) => (
            <button
              key={name}
              aria-label={name}
              onClick={() => navigate(name)}
              aria-current={page === name ? "page" : undefined}
              className={`nav-item ${page === name ? "active" : ""}`}
            >
              <Icon size={17} />
              {name}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="runtime-node">
            <span className="status-dot" />
            <div>
              <strong>Rust execution core</strong>
              <span>One runtime. Clear authority.</span>
            </div>
          </div>
          <button className="help-link" onClick={() => navigate("Settings")}>
            <CircleHelp size={16} /> Runtime guide <ArrowUpRight size={14} />
          </button>
          <div className="profile">
            <span className="profile-avatar">KK</span>
            <div>
              <strong>Workspace owner</strong>
              <span>Local operator</span>
            </div>
            <span className="profile-dot" />
          </div>
        </div>
      </aside>
      <div className="workspace">
        <header className="topbar">
          <div className="breadcrumb">
            <Button
              className="mobile-only"
              variant="ghost"
              size="icon"
              aria-label="Open menu"
              onClick={() => setOpen(true)}
            >
              <Menu size={20} />
            </Button>
            <span>Workspace</span>
            <span className="slash">/</span>
            <strong>{page}</strong>
          </div>
          <div className="topbar-right">
            <span className="network">
              <span className="network-glyph">≋</span> Solana
            </span>
            <div className="mode-switch" role="group" aria-label="Trading mode">
              <button
                aria-pressed={mode === "DRY_RUN"}
                disabled={busy}
                className={mode === "DRY_RUN" ? "selected" : ""}
                onClick={() => onMode("DRY_RUN")}
              >
                Dry Run
              </button>
              <button
                aria-pressed={mode === "LIVE"}
                disabled={busy}
                className={mode === "LIVE" ? "selected live-selected" : ""}
                onClick={() => onMode("LIVE")}
              >
                Live
              </button>
            </div>
            <button
              className="notification-button"
              aria-label="View notifications"
              onClick={() => navigate("Notifications")}
            >
              <Bell size={18} />
            </button>
          </div>
        </header>
        <div className={`safety-strip ${mode === "LIVE" ? "safety-live" : ""}`}>
          <span>
            <ShieldCheck size={14} />
            <strong>
              {mode === "DRY_RUN"
                ? "DRY RUN · NO ON-CHAIN EXECUTION"
                : "LIVE · REAL FUNDS · OWNER AUTHORIZATION REQUIRED"}
            </strong>
            <span className="strip-detail">
              {mode === "DRY_RUN"
                ? "Virtual capital. Live market data. No on-chain execution."
                : "Fresh quote → risk validation → simulation → signing → reconciliation."}
            </span>
          </span>
          <span className="strip-protocol">
            {mode === "DRY_RUN"
              ? "SIMULATION ENVIRONMENT"
              : "PROTECTED EXECUTION"}
          </span>
        </div>
        <main id="main-content" tabIndex={-1}>
          <div className="page-heading">
            <div>
              <div className="eyebrow">YOUR TRADING COMMAND CENTER</div>
              <h1>
                {page === "Overview" ? "Clarity before execution." : page}
              </h1>
              <p>
                {page === "Overview"
                  ? "Every signal, decision, and position. One clear view."
                  : "Inspect the runtime. Stay in control of every decision."}
              </p>
            </div>
            <div className="heading-actions">{actions}</div>
          </div>
          {children}
          <footer className="page-footer">
            <span>
              <span className="footer-cross">✳</span> KAIROS AGENT{" "}
              <span className="muted">/</span> AI proposes. The core decides.
            </span>
            <span>
              V0.1.0 <span className="footer-divider">/</span>{" "}
              {mode === "DRY_RUN"
                ? "LIVE MARKET / VIRTUAL FILLS"
                : "MAINNET · USDC / WSOL"}
            </span>
          </footer>
        </main>
      </div>
    </div>
  );
}
