//! Transport-independent contracts and deterministic financial policy.
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
pub const WSOL: &str = "So11111111111111111111111111111111111111112";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TradingMode {
    DryRun,
    Live,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    pub wallet_required: bool,
    pub signing_enabled: bool,
    pub submission_enabled: bool,
}
impl TradingMode {
    pub fn capabilities(self, live_authorized: bool) -> Capabilities {
        let live = self == Self::Live;
        Capabilities {
            wallet_required: live,
            signing_enabled: live && live_authorized,
            submission_enabled: live && live_authorized,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RiskPolicy {
    pub max_trade_usdc: Decimal,
    pub max_exposure_usdc: Decimal,
    pub daily_loss_usdc: Decimal,
    pub max_positions: usize,
    pub max_slippage_bps: u16,
    pub stop_loss_bps: u16,
    pub take_profit_bps: u16,
    pub trailing_stop_bps: u16,
    pub time_stop_seconds: u64,
}
impl Default for RiskPolicy {
    fn default() -> Self {
        Self {
            max_trade_usdc: Decimal::from(100),
            max_exposure_usdc: Decimal::from(500),
            daily_loss_usdc: Decimal::from(50),
            max_positions: 5,
            max_slippage_bps: 50,
            stop_loss_bps: 300,
            take_profit_bps: 600,
            trailing_stop_bps: 400,
            time_stop_seconds: 3600,
        }
    }
}
impl RiskPolicy {
    pub fn validate(&self) -> Result<(), String> {
        if self.max_trade_usdc <= Decimal::ZERO
            || self.max_trade_usdc > Decimal::from(10_000)
            || self.max_exposure_usdc < self.max_trade_usdc
            || self.max_exposure_usdc > Decimal::from(100_000)
            || self.daily_loss_usdc <= Decimal::ZERO
            || self.daily_loss_usdc > self.max_exposure_usdc
            || !(1..=20).contains(&self.max_positions)
            || !(1..=100).contains(&self.max_slippage_bps)
            || !(1..=5000).contains(&self.stop_loss_bps)
            || !(1..=10000).contains(&self.take_profit_bps)
            || !(1..=5000).contains(&self.trailing_stop_bps)
            || !(60..=86400).contains(&self.time_stop_seconds)
            || [
                self.max_trade_usdc,
                self.max_exposure_usdc,
                self.daily_loss_usdc,
            ]
            .iter()
            .any(|v| v.scale() > 6)
        {
            return Err(
                "Invalid risk policy: check positive limits, exposure, precision and exit rules"
                    .into(),
            );
        }
        Ok(())
    }
    pub fn approve(&self, input: &RiskInput) -> Result<ApprovedIntent, String> {
        self.validate()?;
        let reason = if input.paused {
            Some("Entries are paused or locked")
        } else if !input.data_fresh {
            Some("Market data or quote is stale")
        } else if !input.token_safe {
            Some("Token or route safety checks failed")
        } else if input.notional <= Decimal::ZERO || input.notional > self.max_trade_usdc {
            Some("Maximum trade notional exceeded")
        } else if input.open_positions >= self.max_positions {
            Some("Maximum open positions reached")
        } else if input.exposure + input.notional > self.max_exposure_usdc {
            Some("Portfolio exposure limit reached")
        } else if input.realized_pnl <= -self.daily_loss_usdc {
            Some("Loss limit reached")
        } else if input.notional + input.fee > input.available_balance {
            Some("Insufficient available balance after fees")
        } else {
            None
        };
        if let Some(reason) = reason {
            return Err(reason.into());
        }
        Ok(ApprovedIntent {
            mode: input.mode,
            scope_id: input.scope_id.clone(),
            notional: input.notional,
            exit_atoms: None,
        })
    }
    pub fn approve_exit(
        &self,
        mode: TradingMode,
        scope_id: &str,
        atoms: u64,
        confirmed_atoms: u64,
    ) -> Result<ApprovedIntent, String> {
        self.validate()?;
        if scope_id.is_empty() || atoms == 0 || atoms > confirmed_atoms {
            return Err("Exit quantity exceeds the confirmed position allocation".into());
        }
        Ok(ApprovedIntent {
            mode,
            scope_id: scope_id.into(),
            notional: Decimal::ZERO,
            exit_atoms: Some(atoms),
        })
    }
}
pub struct RiskInput {
    pub mode: TradingMode,
    pub scope_id: String,
    pub paused: bool,
    pub data_fresh: bool,
    pub token_safe: bool,
    pub notional: Decimal,
    pub fee: Decimal,
    pub open_positions: usize,
    pub exposure: Decimal,
    pub realized_pnl: Decimal,
    pub available_balance: Decimal,
}
/// Only the deterministic risk engine can construct this authorization.
pub struct ApprovedIntent {
    mode: TradingMode,
    scope_id: String,
    notional: Decimal,
    exit_atoms: Option<u64>,
}
impl ApprovedIntent {
    pub fn mode(&self) -> TradingMode {
        self.mode
    }
    pub fn scope_id(&self) -> &str {
        &self.scope_id
    }
    pub fn notional(&self) -> Decimal {
        self.notional
    }
    pub fn exit_atoms(&self) -> Option<u64> {
        self.exit_atoms
    }
}

pub fn money(value: Decimal) -> Decimal {
    value.round_dp(6)
}
pub fn bps(value: u16) -> Decimal {
    Decimal::from(value) / Decimal::from(10_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dry_run_cannot_gain_signing_capability() {
        let caps = TradingMode::DryRun.capabilities(true);
        assert!(!caps.signing_enabled && !caps.submission_enabled && !caps.wallet_required);
    }
    #[test]
    fn risk_veto_and_exact_amounts() {
        let mut input = RiskInput {
            mode: TradingMode::DryRun,
            scope_id: "test".into(),
            paused: false,
            data_fresh: true,
            token_safe: true,
            notional: Decimal::from(100),
            fee: Decimal::new(1, 1),
            open_positions: 0,
            exposure: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            available_balance: Decimal::new(1001, 1),
        };
        assert!(RiskPolicy::default().approve(&input).is_ok());
        input.available_balance -= Decimal::new(1, 6);
        assert!(RiskPolicy::default().approve(&input).is_err());
        input.available_balance = Decimal::from(1000);
        input.data_fresh = false;
        assert!(RiskPolicy::default().approve(&input).is_err());
    }
}
