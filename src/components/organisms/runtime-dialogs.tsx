import { useState, type FormEvent } from "react";
import { ArrowRight, LockKeyhole, ShieldCheck } from "lucide-react";
import type { Action, Agent, Position, Snapshot } from "../../lib/contracts";
import { usd } from "../../lib/format";
import { Button } from "../atoms/button";
import { Badge } from "../atoms/badge";
import { Field } from "../molecules/field";
import { Modal } from "../molecules/modal";

export type DialogState =
  | "session"
  | "configure"
  | "unlock"
  | "activate"
  | { agent: Agent }
  | { close: Position }
  | null;
export function RuntimeDialogs({
  dialog,
  onClose,
  command,
  busy,
  snapshot,
}: {
  dialog: DialogState;
  onClose: () => void;
  command: (action: Action) => Promise<unknown>;
  busy: boolean;
  snapshot: Snapshot;
}) {
  const [error, setError] = useState<string | null>(null);
  const close = () => {
    setError(null);
    onClose();
  };
  async function execute(action: Action) {
    setError(null);
    try {
      await command(action);
      close();
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    }
  }
  const submit =
    (handler: (data: FormData) => Action) =>
    (event: FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      void execute(handler(new FormData(event.currentTarget)));
    };
  const get = (data: FormData, key: string) => String(data.get(key) ?? "");
  const failure = error && (
    <div className="form-error" role="alert">
      {error}
    </div>
  );
  if (!dialog) return null;
  if (typeof dialog === "object" && "agent" in dialog) {
    const agent = dialog.agent;
    return (
      <Modal
        open
        onClose={close}
        title={agent.name}
        description="Execution evidence from the runtime; agent output never grants signing authority."
      >
        <div className="detail-stack">
          <Badge tone="purple">{agent.status}</Badge>
          <dl className="detail-list">
            <dt>Role</dt>
            <dd>{agent.role}</dd>
            <dt>Model / source</dt>
            <dd>{agent.model}</dd>
            <dt>Cycle</dt>
            <dd className="mono">{agent.cycle_id ?? "No cycle yet"}</dd>
          </dl>
          <div className="info-box">
            <div style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>
              {agent.output}
            </div>
          </div>
        </div>
      </Modal>
    );
  }
  if (typeof dialog === "object" && "close" in dialog) {
    const p = dialog.close;
    return (
      <Modal
        open
        onClose={close}
        title={`Close ${p.symbol} position`}
        description={
          p.mode === "LIVE"
            ? "This requests a real sale of this position. The backend obtains a fresh quote and validates the exit before signing."
            : "This creates a simulated exit fill and returns the proceeds to your virtual account."
        }
      >
        <div className="info-box">
          Entry cost: <strong>{usd(p.cost_basis)}</strong>
          <br />
          Mode: <strong>{p.mode}</strong>
        </div>
        {failure}
        <div className="form-actions">
          <Button variant="outline" onClick={close}>
            Cancel
          </Button>
          <Button
            disabled={busy}
            onClick={() => void execute({ action: "close_position", id: p.id })}
          >
            {busy ? "Validating…" : "Confirm close"}
          </Button>
        </div>
      </Modal>
    );
  }
  if (dialog === "session")
    return (
      <Modal
        open
        onClose={close}
        title="Start with a dry run."
        description="Validate the complete decision-to-fill workflow with an isolated virtual account. No wallet, signatures, or on-chain submission."
      >
        <div className="setup-steps">
          <span className="active">01 / CONFIGURE</span>
          <ArrowRight size={13} />
          <span>02 / RUN & OBSERVE</span>
        </div>
        <form
          onSubmit={submit((data) => ({
            action: "create_session",
            name: get(data, "name"),
            starting_balance: get(data, "capital"),
            fee_bps: Number(get(data, "fee")),
            slippage_bps: Number(get(data, "slippage")),
          }))}
        >
          <Field
            label="Session name"
            name="name"
            defaultValue="My first dry run"
            required
            maxLength={80}
          />
          <Field
            label="Virtual starting capital (USDC)"
            name="capital"
            type="number"
            min="1"
            max="1000000"
            step="0.000001"
            defaultValue="10000"
            required
          />
          <div className="form-grid">
            <Field
              label="Fee model (basis points)"
              name="fee"
              type="number"
              min="0"
              max="100"
              defaultValue="10"
              required
            />
            <Field
              label="Slippage (basis points)"
              name="slippage"
              type="number"
              min="0"
              max={snapshot.policy.max_slippage_bps}
              defaultValue="20"
              required
            />
          </div>
          <div className="info-box">
            <ShieldCheck size={18} />
            <div>
              <strong>Live market · virtual execution</strong>
              <p>
                This session uses current Jupiter prices and swap quotes. AI
                decisions read prior trade logs. Only the balance and fills are
                simulated.
              </p>
            </div>
          </div>
          {failure}
          <div className="form-actions">
            <span className="muted">Wallet not required</span>
            <Button type="submit" disabled={busy}>
              {busy ? "Creating…" : "Create session"}
              <ArrowRight size={15} />
            </Button>
          </div>
        </form>
      </Modal>
    );
  if (dialog === "configure")
    return (
      <Modal
        open
        onClose={close}
        title="Configure live execution"
        description="Configure a dedicated Solana wallet and two independent mainnet RPC providers. The backend encrypts all credentials with your passphrase."
      >
        <form
          onSubmit={submit((data) => ({
            action: "configure_live",
            rpc_url: get(data, "rpc"),
            secondary_rpc_url: get(data, "secondary"),
            jupiter_api_key: get(data, "jupiter"),
            keypair_json: get(data, "keypair") || null,
            passphrase: get(data, "passphrase"),
          }))}
        >
          <Field
            label="Primary RPC endpoint"
            name="rpc"
            type="url"
            placeholder="https://your-primary-rpc.example"
            autoComplete="off"
            required
          />
          <Field
            label="Secondary RPC endpoint"
            name="secondary"
            type="url"
            placeholder="https://your-secondary-rpc.example"
            autoComplete="off"
            required
          />
          <Field
            label="Jupiter API key"
            name="jupiter"
            type="password"
            autoComplete="new-password"
            required
          />
          <Field
            label="Dedicated keypair JSON (optional)"
            name="keypair"
            type="password"
            autoComplete="new-password"
            hint="Leave blank to generate a new dedicated wallet. Never enter a main-wallet seed phrase."
          />
          <Field
            label="Vault passphrase"
            name="passphrase"
            type="password"
            autoComplete="new-password"
            minLength={12}
            maxLength={256}
            required
            hint="Keep this passphrase safe. It is required to unlock the encrypted vault."
          />
          <div className="info-box">
            <LockKeyhole size={18} />
            <div>
              <strong>Restricted live route</strong>
              <p>
                USDC ↔ wrapped SOL through Orca Whirlpool. Both associated token
                accounts must already exist. Keep at least 0.0201 SOL for
                network fees.
              </p>
            </div>
          </div>
          {failure}
          <div className="form-actions">
            <Button variant="outline" onClick={close}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy}>
              {busy ? "Encrypting…" : "Encrypt & save"}
            </Button>
          </div>
        </form>
      </Modal>
    );
  if (dialog === "unlock")
    return (
      <Modal
        open
        onClose={close}
        title="Unlock the signer"
        description="Your passphrase unlocks the vault in the Rust runtime. Unlocking does not activate trading."
      >
        <form
          onSubmit={submit((data) => ({
            action: "unlock_wallet",
            passphrase: get(data, "passphrase"),
          }))}
        >
          <Field
            label="Vault passphrase"
            name="passphrase"
            type="password"
            autoComplete="current-password"
            maxLength={256}
            required
            autoFocus
          />
          {failure}
          <div className="form-actions">
            <Button variant="outline" onClick={close}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy}>
              Unlock wallet
            </Button>
          </div>
        </form>
      </Modal>
    );
  if (dialog === "activate")
    return (
      <Modal
        open
        onClose={close}
        title="Activate live trading"
        description="Live trades move real funds. Activation lasts 15 minutes and every entry requires a separate proposal approval."
      >
        <form
          onSubmit={submit((data) => ({
            action: "activate_live",
            confirmation: get(data, "confirmation"),
          }))}
        >
          <div className="info-box">
            <div>
              <strong>
                {usd(snapshot.policy.max_trade_usdc)} maximum per trade
              </strong>
              <p>
                Exposure limit {usd(snapshot.policy.max_exposure_usdc)} · loss
                ceiling {usd(snapshot.policy.daily_loss_usdc)} · slippage{" "}
                {snapshot.policy.max_slippage_bps} bps. The backend verifies
                provider readiness, fee reserve, reconciliation and the operator
                validation gate.
              </p>
            </div>
          </div>
          <Field
            label="Type ACTIVATE LIVE to confirm"
            name="confirmation"
            placeholder="ACTIVATE LIVE"
            autoComplete="off"
            pattern="ACTIVATE LIVE"
            required
          />
          {failure}
          <div className="form-actions">
            <Button variant="outline" onClick={close}>
              Cancel
            </Button>
            <Button type="submit" disabled={busy}>
              Authorize live session
            </Button>
          </div>
        </form>
      </Modal>
    );
  return null;
}
