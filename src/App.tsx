import { useState, type FormEvent } from "react";
import {
  Command,
  LoaderCircle,
  Pause,
  Play,
  Plus,
  ShieldAlert,
  X,
} from "lucide-react";
import type { Action, Page } from "./lib/contracts";
import { ApiError, login } from "./lib/api";
import { useRuntime } from "./hooks/use-runtime";
import { Button } from "./components/atoms/button";
import { Badge } from "./components/atoms/badge";
import { Field } from "./components/molecules/field";
import { CommandLayout } from "./components/templates/command-layout";
import {
  RuntimeDialogs,
  type DialogState,
} from "./components/organisms/runtime-dialogs";
import { Overview } from "./pages/overview";
import { WorkspacePage } from "./pages/workspace-page";
import "./App.css";

export default function App() {
  const runtime = useRuntime();
  const [page, setPage] = useState<Page>("Overview");
  const [dialog, setDialog] = useState<DialogState>(null);
  const [loginError, setLoginError] = useState("");
  const [loggingIn, setLoggingIn] = useState(false);
  async function signIn(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const token = String(new FormData(event.currentTarget).get("token"));
    setLoggingIn(true);
    try {
      await login(token);
      setLoginError("");
      await runtime.refetch();
    } catch (error) {
      setLoginError(error instanceof Error ? error.message : String(error));
    } finally {
      setLoggingIn(false);
    }
  }
  if (
    !runtime.data ||
    (runtime.error instanceof ApiError && runtime.error.status === 401)
  ) {
    const unauthenticated =
      runtime.error instanceof ApiError && runtime.error.status === 401;
    return (
      <div className="welcome-screen">
        <div className="welcome-art">
          <span className="welcome-brand">
            <Command size={24} /> kairos<span>agent</span>
          </span>
          <div>
            <span className="eyebrow">BUILT FOR DELIBERATE EXECUTION</span>
            <h1>
              A clear view.
              <br />A controlled edge.
            </h1>
            <p>
              Your Solana trading command center.
              <br />
              From the first dry run to protected live execution.
            </p>
          </div>
          <span className="mono">AI PROPOSES. THE CORE DECIDES.</span>
        </div>
        <main className="welcome-form">
          <Badge tone="purple">PRIVATE WORKSPACE</Badge>
          <h2>
            {unauthenticated
              ? "Your runtime. Your control."
              : "Connecting to your runtime."}
          </h2>
          <p>
            {unauthenticated
              ? "Sign in with the owner token saved in your runtime data directory."
              : "Start the Rust server with npm run server, or open the desktop application."}
          </p>
          {unauthenticated ? (
            <form onSubmit={signIn}>
              <Field
                label="Owner token"
                name="token"
                type="password"
                autoComplete="current-password"
                required
                autoFocus
              />
              {loginError && (
                <p className="form-error" role="alert">
                  {loginError}
                </p>
              )}
              <Button type="submit" disabled={loggingIn}>
                {loggingIn ? "Signing in…" : "Open command center"}
              </Button>
              <small className="login-hint">
                Local token: .kairos-data/owner-token
              </small>
            </form>
          ) : (
            <>
              <p className="muted">
                {runtime.isLoading ? (
                  <LoaderCircle className="spin" size={22} />
                ) : (
                  runtime.error?.message
                )}
              </p>
              <Button variant="outline" onClick={() => void runtime.refetch()}>
                Retry connection
              </Button>
            </>
          )}
        </main>
      </div>
    );
  }
  const snapshot = runtime.data;
  const dry = snapshot.mode === "DRY_RUN";
  const act = (action: Action) => {
    void runtime.command(action).catch(() => {});
  };
  const actionProps = {
    snapshot,
    command: act,
    busy: runtime.busy,
    onAgent: (agent: SnapshotAgent) => setDialog({ agent }),
    onClose: (position: SnapshotPosition) => setDialog({ close: position }),
  };
  return (
    <CommandLayout
      page={page}
      onNavigate={setPage}
      mode={snapshot.mode}
      busy={runtime.busy}
      onMode={(mode) => {
        if (mode !== snapshot.mode) act({ action: "set_mode", mode });
      }}
      actions={
        <>
          <Badge
            dot
            tone={
              snapshot.killed
                ? "danger"
                : snapshot.paused
                  ? "neutral"
                  : "positive"
            }
          >
            {snapshot.killed
              ? "LOCKED"
              : snapshot.paused
                ? "PAUSED"
                : "RUNNING"}
          </Badge>
          <Button
            variant="outline"
            disabled={runtime.busy || (!snapshot.session && dry)}
            onClick={() =>
              act({ action: snapshot.paused ? "resume" : "pause" })
            }
          >
            {snapshot.paused ? <Play size={14} /> : <Pause size={14} />}{" "}
            {snapshot.paused ? "Resume" : "Pause"}
          </Button>
          {dry ? (
            !snapshot.session || snapshot.session.status === "Stopped" ? (
              <Button
                disabled={runtime.busy}
                onClick={() => setDialog("session")}
              >
                <Plus size={15} />
                New dry run
              </Button>
            ) : snapshot.session.status === "Ready" ? (
              <Button
                disabled={runtime.busy}
                onClick={() => act({ action: "start_session" })}
              >
                <Play size={14} />
                Start session
              </Button>
            ) : (
              <Button
                disabled={runtime.busy || snapshot.paused || snapshot.killed}
                onClick={() => act({ action: "run_cycle" })}
              >
                <Play size={14} />
                Run cycle
              </Button>
            )
          ) : (
            <Button
              disabled={runtime.busy}
              onClick={() =>
                snapshot.capabilities.signing_enabled
                  ? act({ action: "run_cycle" })
                  : setDialog(
                      !snapshot.live.public_key
                        ? "configure"
                        : !snapshot.live.unlocked
                          ? "unlock"
                          : "activate",
                    )
              }
            >
              {snapshot.capabilities.signing_enabled
                ? "Run live cycle"
                : !snapshot.live.public_key
                  ? "Set up live"
                  : !snapshot.live.unlocked
                    ? "Unlock wallet"
                    : "Activate live"}
            </Button>
          )}
          <Button
            variant="destructive"
            size="icon"
            aria-label="Activate kill switch"
            title="Stop new signing and submission immediately"
            disabled={snapshot.killed}
            onClick={() => act({ action: "kill" })}
          >
            <ShieldAlert size={17} />
          </Button>
        </>
      }
    >
      {runtime.isError && (
        <div role="alert" className="connection-error">
          Runtime connection lost. Displayed data may be stale. Reconnect before
          issuing commands.
        </div>
      )}
      {runtime.notice && (
        <div
          className={`notice ${runtime.notice.error ? "notice-error" : ""}`}
          role={runtime.notice.error ? "alert" : "status"}
        >
          <span>{runtime.notice.text}</span>
          <button
            aria-label="Dismiss notification"
            onClick={runtime.clearNotice}
          >
            <X size={16} />
          </button>
        </div>
      )}
      {page === "Overview" ? (
        <Overview {...actionProps} onNavigate={setPage} />
      ) : (
        <WorkspacePage
          key={page}
          {...actionProps}
          page={page}
          onDialog={setDialog}
        />
      )}
      <RuntimeDialogs
        key={
          typeof dialog === "string"
            ? dialog
            : dialog
              ? JSON.stringify(dialog)
              : "closed"
        }
        dialog={dialog}
        onClose={() => setDialog(null)}
        command={runtime.command}
        busy={runtime.busy}
        snapshot={snapshot}
      />
    </CommandLayout>
  );
}
type SnapshotAgent = import("./lib/contracts").Agent;
type SnapshotPosition = import("./lib/contracts").Position;
