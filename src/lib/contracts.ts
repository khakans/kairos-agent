import { z } from "zod";

const amount = z.string().regex(/^-?\d+(\.\d+)?$/);
export const modeSchema = z.enum(["DRY_RUN", "LIVE"]);
export const policySchema = z.object({
  max_trade_usdc: amount,
  max_exposure_usdc: amount,
  daily_loss_usdc: amount,
  max_positions: z.number().int(),
  max_slippage_bps: z.number().int(),
  stop_loss_bps: z.number().int(),
  take_profit_bps: z.number().int(),
  trailing_stop_bps: z.number().int(),
  time_stop_seconds: z.number().int(),
});
const sessionSchema = z.object({
  id: z.string(),
  parent_session_id: z.string().nullable(),
  name: z.string(),
  status: z.string(),
  starting_balance: amount,
  balance: amount,
  fee_bps: z.number(),
  slippage_bps: z.number(),
  created_at: z.number(),
  policy: policySchema,
  source: z.string(),
});
const marketSchema = z.object({
  symbol: z.string(),
  name: z.string(),
  price: amount,
  change_bps: z.number(),
  history: z.array(amount),
  source: z.string(),
  updated_at: z.number(),
});
const positionSchema = z.object({
  id: z.string(),
  scope_id: z.string(),
  mode: modeSchema,
  symbol: z.string(),
  quantity: amount,
  entry_price: amount,
  mark_price: amount,
  high_price: amount,
  cost_basis: amount,
  realized_pnl: amount,
  status: z.string(),
  opened_at: z.number(),
  exit_reason: z.string().nullable(),
  policy: policySchema,
});
const orderSchema = z.object({
  id: z.string(),
  scope_id: z.string(),
  mode: modeSchema,
  symbol: z.string(),
  side: z.string(),
  quantity: amount,
  price: amount,
  fee_usdc: amount,
  fee_lamports: z.number(),
  signature: z.string().nullable(),
  timestamp: z.number(),
  reason: z.string(),
});
const proposalSchema = z.object({
  position_id: z.string().nullable(),
  decision_id: z.string().nullable(),
  id: z.string(),
  scope_id: z.string(),
  mode: modeSchema,
  symbol: z.string(),
  notional: amount,
  thesis: z.string(),
  status: z.string(),
  reason: z.string(),
  created_at: z.number(),
  expires_at: z.number(),
});
const agentSchema = z.object({
  id: z.string(),
  name: z.string(),
  role: z.string(),
  status: z.string(),
  output: z.string(),
  model: z.string(),
  cycle_id: z.string().nullable(),
  completed_at: z.number().nullable(),
});
const eventSchema = z.object({
  id: z.string(),
  sequence: z.number(),
  mode: modeSchema,
  scope_id: z.string().nullable(),
  timestamp: z.number(),
  kind: z.string(),
  message: z.string(),
  correlation_id: z.string(),
});
export const snapshotSchema = z.object({
  schema_version: z.literal(1),
  market_error: z.string().nullable(),
  market_slot: z.number(),
  mode: modeSchema,
  paused: z.boolean(),
  killed: z.boolean(),
  session: sessionSchema.nullable(),
  archived_sessions: z.array(sessionSchema),
  live: z.object({
    public_key: z.string().nullable(),
    wallet: z
      .object({
        public_key: z.string(),
        sol_lamports: z.number(),
        usdc_atoms: z.number(),
        wsol_atoms: z.number(),
        slot: z.number(),
        checked_at: z.number(),
      })
      .nullable(),
    pending: z
      .object({ signature: z.string(), submitted_at: z.number() })
      .passthrough()
      .nullable(),
    pending_position_id: z.string().nullable(),
    activation_expires_at: z.number(),
    readiness_error: z.string().nullable(),
    unlocked: z.boolean(),
  }),
  policy: policySchema,
  markets: z.array(marketSchema),
  positions: z.array(positionSchema),
  orders: z.array(orderSchema),
  proposals: z.array(proposalSchema),
  agents: z.array(agentSchema),
  events: z.array(eventSchema),
  sequence: z.number(),
  cycle_count: z.number(),
  portfolio: z.object({
    equity: amount,
    cash: amount,
    exposure: amount,
    realized_pnl: amount,
    unrealized_pnl: amount,
    fees: amount,
    open_positions: z.number(),
  }),
  readiness: z.array(
    z.object({
      capability: z.string(),
      requirement: z.string(),
      status: z.string(),
      detail: z.string(),
    }),
  ),
  capabilities: z.object({
    wallet_required: z.boolean(),
    signing_enabled: z.boolean(),
    submission_enabled: z.boolean(),
  }),
});
export type Snapshot = z.infer<typeof snapshotSchema>;
export type Mode = z.infer<typeof modeSchema>;
export type Policy = z.infer<typeof policySchema>;
export type Market = z.infer<typeof marketSchema>;
export type Position = z.infer<typeof positionSchema>;
export type Agent = z.infer<typeof agentSchema>;
export type Action =
  | {
      action: "create_session";
      name: string;
      starting_balance: string;
      fee_bps: number;
      slippage_bps: number;
    }
  | {
      action:
        | "start_session"
        | "pause"
        | "resume"
        | "stop_session"
        | "reset_session"
        | "kill"
        | "run_cycle"
        | "lock_wallet"
        | "refresh_wallet"
        | "reconcile";
    }
  | { action: "set_mode"; mode: Mode }
  | {
      action: "approve_proposal" | "reject_proposal" | "close_position";
      id: string;
    }
  | { action: "update_policy"; policy: Policy }
  | {
      action: "configure_live";
      rpc_url: string;
      secondary_rpc_url: string;
      jupiter_api_key: string;
      keypair_json: string | null;
      passphrase: string;
    }
  | { action: "unlock_wallet"; passphrase: string }
  | { action: "activate_live"; confirmation: string };
export type Page =
  | "Overview"
  | "Agent Field"
  | "Markets"
  | "Signals"
  | "Portfolio"
  | "Positions"
  | "Orders"
  | "Risk"
  | "Adapters"
  | "Wallets"
  | "Notifications"
  | "Audit Log"
  | "Settings";
export function scopeOf(snapshot: Snapshot) {
  return snapshot.mode === "DRY_RUN"
    ? snapshot.session?.id
    : snapshot.live.public_key;
}
