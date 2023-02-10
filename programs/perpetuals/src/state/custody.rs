use {
    crate::state::{
        oracle::OracleType,
        perpetuals::{Permissions, Perpetuals},
    },
    anchor_lang::prelude::*,
};

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Debug)]
pub enum FeesMode {
    Fixed,
    Linear,
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct Fees {
    pub mode: FeesMode,
    // fees have implied BPS_DECIMALS decimals
    pub max_increase: u64,
    pub max_decrease: u64,
    pub swap: u64,
    pub add_liquidity: u64,
    pub remove_liquidity: u64,
    pub open_position: u64,
    pub close_position: u64,
    pub liquidation: u64,
    pub protocol_share: u64,
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct FeesStats {
    pub swap_usd: u64,
    pub add_liquidity_usd: u64,
    pub remove_liquidity_usd: u64,
    pub open_position_usd: u64,
    pub close_position_usd: u64,
    pub liquidation_usd: u64,
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct VolumeStats {
    pub swap_usd: u64,
    pub add_liquidity_usd: u64,
    pub remove_liquidity_usd: u64,
    pub open_position_usd: u64,
    pub close_position_usd: u64,
    pub liquidation_usd: u64,
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct TradeStats {
    pub profit_usd: u64,
    pub loss_usd: u64,
    // open interest
    pub oi_long_usd: u64,
    pub oi_short_usd: u64,
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct Assets {
    pub collateral: u64,
    // protocol_fees are part of the collected fees that is reserved for the protocol
    pub protocol_fees: u64,
    // owned = total_assets - collateral + collected_fees - protocol_fees
    pub owned: u64,
    // locked funds for pnl payoff
    pub locked: u64,
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct OracleParams {
    pub oracle_account: Pubkey,
    pub oracle_type: OracleType,
    pub max_price_error: u64,
    pub max_price_age_sec: u32,
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct PricingParams {
    pub use_ema: bool,
    // pricing params have implied BPS_DECIMALS decimals
    pub trade_spread_long: u64,
    pub trade_spread_short: u64,
    pub swap_spread: u64,
    pub min_initial_leverage: u64,
    pub max_leverage: u64,
}

#[account]
#[derive(Default, Debug)]
pub struct Custody {
    pub pool: Pubkey,
    pub mint: Pubkey,
    pub token_account: Pubkey,
    pub decimals: u8,
    pub is_stable: bool,
    pub oracle: OracleParams,
    pub pricing: PricingParams,
    pub permissions: Permissions,
    pub fees: Fees,
    // borrow rates have implied RATE_DECIMALS decimals
    pub borrow_rate: u64,
    pub borrow_rate_sum: u64,

    pub assets: Assets,
    pub collected_fees: FeesStats,
    pub volume_stats: VolumeStats,
    pub trade_stats: TradeStats,

    pub bump: u8,
    pub token_account_bump: u8,
}

#[account]
#[derive(Default, Debug)]
pub struct DeprecatedCustody {
    pub token_account: Pubkey,
    pub mint: Pubkey,
    pub decimals: u8,
    pub oracle: OracleParams,
    pub pricing: PricingParams,
    pub permissions: Permissions,
    pub fees: Fees,
    pub borrow_rate: u64,
    pub borrow_rate_sum: u64,

    pub assets: Assets,
    pub collected_fees: FeesStats,
    pub volume_stats: VolumeStats,
    pub trade_stats: TradeStats,

    pub bump: u8,
    pub token_account_bump: u8,
}

impl Default for FeesMode {
    fn default() -> Self {
        Self::Linear
    }
}

impl Fees {
    pub fn validate(&self) -> bool {
        self.max_decrease as u128 <= Perpetuals::BPS_POWER
            && self.swap as u128 <= Perpetuals::BPS_POWER
            && self.add_liquidity as u128 <= Perpetuals::BPS_POWER
            && self.remove_liquidity as u128 <= Perpetuals::BPS_POWER
            && self.open_position as u128 <= Perpetuals::BPS_POWER
            && self.close_position as u128 <= Perpetuals::BPS_POWER
            && self.liquidation as u128 <= Perpetuals::BPS_POWER
            && self.protocol_share as u128 <= Perpetuals::BPS_POWER
    }
}

impl OracleParams {
    pub fn validate(&self) -> bool {
        self.oracle_type == OracleType::None || self.oracle_account != Pubkey::default()
    }
}

impl PricingParams {
    pub fn validate(&self) -> bool {
        self.min_initial_leverage <= self.max_leverage
            && (self.trade_spread_long as u128) < Perpetuals::BPS_POWER
            && (self.trade_spread_short as u128) < Perpetuals::BPS_POWER
            && (self.swap_spread as u128) < Perpetuals::BPS_POWER
    }
}

impl Custody {
    pub const LEN: usize = 8 + std::mem::size_of::<Custody>();

    pub fn validate(&self) -> bool {
        self.token_account != Pubkey::default()
            && self.mint != Pubkey::default()
            && self.oracle.validate()
            && self.pricing.validate()
            && self.fees.validate()
    }
}
