use {
    crate::{
        error::PerpetualsError,
        math,
        state::{
            custody::{Custody, FeesMode},
            oracle::OraclePrice,
            perpetuals::Perpetuals,
            position::{Position, Side},
        },
    },
    anchor_lang::prelude::*,
    std::cmp::Ordering,
};

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Debug)]
pub enum AumCalcMode {
    Min,
    Max,
    Last,
    EMA,
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct TokenRatios {
    pub target: u64,
    pub min: u64,
    pub max: u64,
}

#[account]
#[derive(Default, Debug)]
pub struct Pool {
    pub name: String,
    pub custodies: Vec<Pubkey>,
    pub ratios: Vec<TokenRatios>,
    pub aum_usd: u128,

    pub bump: u8,
    pub lp_token_bump: u8,
    pub inception_time: i64,
}

impl TokenRatios {
    pub fn validate(&self) -> bool {
        (self.target as u128) <= Perpetuals::BPS_POWER
            && (self.min as u128) <= Perpetuals::BPS_POWER
            && (self.max as u128) <= Perpetuals::BPS_POWER
            && self.min <= self.target
            && self.target <= self.max
    }
}

/// Token Pool
/// All returned prices are scaled to PRICE_DECIMALS.
/// All returned amounts are scaled to corresponding custody decimals.
///
impl Pool {
    pub const LEN: usize = 8 + 64 + std::mem::size_of::<Pool>();

    pub fn validate(&self) -> bool {
        for ratio in &self.ratios {
            if !ratio.validate() {
                return false;
            }
        }

        // check target ratios add up to 1
        if !self.ratios.is_empty()
            && self
                .ratios
                .iter()
                .map(|&x| (x.target as u128))
                .sum::<u128>()
                != Perpetuals::BPS_POWER
        {
            return false;
        }

        // check custodies are unique
        for i in 1..self.custodies.len() {
            if self.custodies[i..].contains(&self.custodies[i - 1]) {
                return false;
            }
        }

        !self.name.is_empty() && self.name.len() <= 64 && self.custodies.len() == self.ratios.len()
    }

    pub fn get_token_id(&self, custody: &Pubkey) -> Result<usize> {
        self.custodies
            .iter()
            .position(|&k| k == *custody)
            .ok_or_else(|| PerpetualsError::UnsupportedToken.into())
    }

    pub fn get_entry_price(
        &self,
        token_price: &OraclePrice,
        token_ema_price: &OraclePrice,
        side: Side,
        custody: &Custody,
    ) -> Result<u64> {
        let price = self.get_price(
            token_price,
            token_ema_price,
            side,
            if side == Side::Long {
                custody.pricing.trade_spread_long
            } else {
                custody.pricing.trade_spread_short
            },
        )?;
        require_gt!(price.price, 0, PerpetualsError::MaxPriceSlippage);

        Ok(price
            .scale_to_exponent(-(Perpetuals::PRICE_DECIMALS as i32))?
            .price)
    }

    pub fn get_entry_fee(
        &self,
        base_fee: u64,
        size: u64,
        locked_amount: u64,
        collateral_custody: &Custody,
    ) -> Result<u64> {
        // entry_fee = custody.fees.open_position * utilization_fee * size
        // where utilization_fee = 1 + custody.fees.utilization_mult * (new_utilization - optimal_utilization) / (1 - optimal_utilization);

        let mut size_fee = Self::get_fee_amount(base_fee, size)?;

        let new_utilization = if collateral_custody.assets.owned > 0 {
            // utilization = (assets_locked + locked_amount) / assets_owned
            std::cmp::min(
                Perpetuals::RATE_POWER,
                math::checked_div(
                    math::checked_mul(
                        math::checked_add(collateral_custody.assets.locked, locked_amount)? as u128,
                        Perpetuals::RATE_POWER,
                    )?,
                    collateral_custody.assets.owned as u128,
                )?,
            )
        } else {
            Perpetuals::RATE_POWER
        };

        if new_utilization > collateral_custody.borrow_rate.optimal_utilization as u128 {
            let utilization_fee = math::checked_add(
                Perpetuals::BPS_POWER,
                math::checked_div(
                    math::checked_mul(
                        collateral_custody.fees.utilization_mult as u128,
                        math::checked_sub(
                            new_utilization,
                            collateral_custody.borrow_rate.optimal_utilization as u128,
                        )?,
                    )?,
                    math::checked_sub(
                        Perpetuals::RATE_POWER,
                        collateral_custody.borrow_rate.optimal_utilization as u128,
                    )?,
                )?,
            )?;
            size_fee = math::checked_as_u64(math::checked_div(
                math::checked_mul(size_fee as u128, utilization_fee)?,
                Perpetuals::BPS_POWER,
            )?)?;
        }

        Ok(size_fee)
    }

    pub fn get_exit_price(
        &self,
        token_price: &OraclePrice,
        token_ema_price: &OraclePrice,
        side: Side,
        custody: &Custody,
    ) -> Result<u64> {
        let price = self.get_price(
            token_price,
            token_ema_price,
            if side == Side::Long {
                Side::Short
            } else {
                Side::Long
            },
            if side == Side::Long {
                custody.pricing.trade_spread_short
            } else {
                custody.pricing.trade_spread_long
            },
        )?;

        Ok(price
            .scale_to_exponent(-(Perpetuals::PRICE_DECIMALS as i32))?
            .price)
    }

    pub fn get_exit_fee(&self, size: u64, custody: &Custody) -> Result<u64> {
        Self::get_fee_amount(custody.fees.close_position, size)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_close_amount(
        &self,
        position: &Position,
        token_price: &OraclePrice,
        token_ema_price: &OraclePrice,
        custody: &Custody,
        collateral_token_price: &OraclePrice,
        collateral_token_ema_price: &OraclePrice,
        collateral_custody: &Custody,
        curtime: i64,
        liquidation: bool,
    ) -> Result<(u64, u64, u64, u64)> {
        let (profit_usd, loss_usd, fee_amount) = self.get_pnl_usd(
            position,
            token_price,
            token_ema_price,
            custody,
            collateral_token_price,
            collateral_token_ema_price,
            collateral_custody,
            curtime,
            liquidation,
        )?;

        let available_amount_usd = if profit_usd > 0 {
            math::checked_add(position.collateral_usd, profit_usd)?
        } else if loss_usd < position.collateral_usd {
            math::checked_sub(position.collateral_usd, loss_usd)?
        } else {
            0
        };

        let max_collateral_price = if collateral_token_price > collateral_token_ema_price {
            collateral_token_price
        } else {
            collateral_token_ema_price
        };
        let close_amount = max_collateral_price
            .get_token_amount(available_amount_usd, collateral_custody.decimals)?;
        let max_amount = math::checked_add(
            position.locked_amount.saturating_sub(fee_amount),
            position.collateral_amount,
        )?;

        Ok((
            std::cmp::min(max_amount, close_amount),
            fee_amount,
            profit_usd,
            loss_usd,
        ))
    }

    pub fn get_swap_price(
        &self,
        token_in_price: &OraclePrice,
        token_in_ema_price: &OraclePrice,
        token_out_price: &OraclePrice,
        token_out_ema_price: &OraclePrice,
        custody_in: &Custody,
    ) -> Result<OraclePrice> {
        let min_price = if token_in_price < token_in_ema_price {
            token_in_price
        } else {
            token_in_ema_price
        };

        let max_price = if token_out_price > token_out_ema_price {
            token_out_price
        } else {
            token_out_ema_price
        };

        let pair_price = min_price.checked_div(max_price)?;

        self.get_price(
            &pair_price,
            &pair_price,
            Side::Short,
            custody_in.pricing.swap_spread,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_swap_amount(
        &self,
        token_in_price: &OraclePrice,
        token_in_ema_price: &OraclePrice,
        token_out_price: &OraclePrice,
        token_out_ema_price: &OraclePrice,
        custody_in: &Custody,
        custody_out: &Custody,
        amount_in: u64,
    ) -> Result<u64> {
        let swap_price = self.get_swap_price(
            token_in_price,
            token_in_ema_price,
            token_out_price,
            token_out_ema_price,
            custody_in,
        )?;

        math::checked_decimal_mul(
            amount_in,
            -(custody_in.decimals as i32),
            swap_price.price,
            swap_price.exponent,
            -(custody_out.decimals as i32),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_swap_fees(
        &self,
        token_id_in: usize,
        token_id_out: usize,
        amount_in: u64,
        amount_out: u64,
        custody_in: &Custody,
        token_price_in: &OraclePrice,
        custody_out: &Custody,
        token_price_out: &OraclePrice,
    ) -> Result<(u64, u64)> {
        let stable_swap = custody_in.is_stable && custody_out.is_stable;

        let swap_in_fee = self.get_fee(
            token_id_in,
            if stable_swap {
                custody_in.fees.stable_swap_in
            } else {
                custody_in.fees.swap_in
            },
            amount_in,
            0u64,
            custody_in,
            token_price_in,
        )?;

        let swap_out_fee = self.get_fee(
            token_id_out,
            if stable_swap {
                custody_out.fees.stable_swap_out
            } else {
                custody_out.fees.swap_out
            },
            0u64,
            amount_out,
            custody_out,
            token_price_out,
        )?;

        Ok((swap_in_fee, swap_out_fee))
    }

    pub fn get_add_liquidity_fee(
        &self,
        token_id: usize,
        amount: u64,
        custody: &Custody,
        token_price: &OraclePrice,
    ) -> Result<u64> {
        self.get_fee(
            token_id,
            custody.fees.add_liquidity,
            amount,
            0u64,
            custody,
            token_price,
        )
    }

    pub fn get_remove_liquidity_fee(
        &self,
        token_id: usize,
        amount: u64,
        custody: &Custody,
        token_price: &OraclePrice,
    ) -> Result<u64> {
        self.get_fee(
            token_id,
            custody.fees.remove_liquidity,
            0u64,
            amount,
            custody,
            token_price,
        )
    }

    pub fn get_liquidation_fee(&self, size: u64, custody: &Custody) -> Result<u64> {
        Self::get_fee_amount(custody.fees.liquidation, size)
    }

    pub fn check_token_ratio(
        &self,
        token_id: usize,
        amount_add: u64,
        amount_remove: u64,
        custody: &Custody,
        token_price: &OraclePrice,
    ) -> Result<bool> {
        let new_ratio = self.get_new_ratio(amount_add, amount_remove, custody, token_price)?;

        if new_ratio < self.ratios[token_id].min {
            Ok(new_ratio >= self.get_current_ratio(custody, token_price)?)
        } else if new_ratio > self.ratios[token_id].max {
            Ok(new_ratio <= self.get_current_ratio(custody, token_price)?)
        } else {
            Ok(true)
        }
    }

    pub fn check_available_amount(&self, amount: u64, custody: &Custody) -> Result<bool> {
        let available_amount = math::checked_sub(
            math::checked_add(custody.assets.owned, custody.assets.collateral)?,
            custody.assets.locked,
        )?;
        Ok(available_amount >= amount)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_leverage(
        &self,
        position: &Position,
        token_price: &OraclePrice,
        token_ema_price: &OraclePrice,
        custody: &Custody,
        collateral_token_price: &OraclePrice,
        collateral_token_ema_price: &OraclePrice,
        collateral_custody: &Custody,
        curtime: i64,
    ) -> Result<u64> {
        let (profit_usd, loss_usd, _) = self.get_pnl_usd(
            position,
            token_price,
            token_ema_price,
            custody,
            collateral_token_price,
            collateral_token_ema_price,
            collateral_custody,
            curtime,
            false,
        )?;

        let current_margin_usd = if profit_usd > 0 {
            math::checked_add(position.collateral_usd, profit_usd)?
        } else if loss_usd <= position.collateral_usd {
            math::checked_sub(position.collateral_usd, loss_usd)?
        } else {
            0
        };

        if current_margin_usd > 0 {
            math::checked_as_u64(math::checked_div(
                math::checked_mul(position.size_usd as u128, Perpetuals::BPS_POWER)?,
                current_margin_usd as u128,
            )?)
        } else {
            Ok(u64::MAX)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn check_leverage(
        &self,
        position: &Position,
        token_price: &OraclePrice,
        token_ema_price: &OraclePrice,
        custody: &Custody,
        collateral_token_price: &OraclePrice,
        collateral_token_ema_price: &OraclePrice,
        collateral_custody: &Custody,
        curtime: i64,
        initial: bool,
    ) -> Result<bool> {
        let current_leverage = self.get_leverage(
            position,
            token_price,
            token_ema_price,
            custody,
            collateral_token_price,
            collateral_token_ema_price,
            collateral_custody,
            curtime,
        )?;

        Ok(current_leverage <= custody.pricing.max_leverage
            && (!initial
                || (current_leverage >= custody.pricing.min_initial_leverage
                    && current_leverage <= custody.pricing.max_initial_leverage)))
    }

    pub fn get_liquidation_price(
        &self,
        position: &Position,
        token_ema_price: &OraclePrice,
        custody: &Custody,
        collateral_custody: &Custody,
        curtime: i64,
    ) -> Result<u64> {
        // liq_price = pos_price +- (collateral + unreal_profit - unreal_loss - exit_fee - interest - size/max_leverage) * pos_price / size

        if position.size_usd == 0 || position.price == 0 {
            return Ok(0);
        }

        let size = token_ema_price.get_token_amount(position.size_usd, custody.decimals)?;
        let exit_fee_tokens = self.get_exit_fee(size, custody)?;
        let exit_fee_usd =
            token_ema_price.get_asset_amount_usd(exit_fee_tokens, custody.decimals)?;
        let interest_usd = collateral_custody.get_interest_amount_usd(position, curtime)?;
        let unrealized_loss_usd = math::checked_add(
            math::checked_add(exit_fee_usd, interest_usd)?,
            position.unrealized_loss_usd,
        )?;

        let max_loss_usd = math::checked_as_u64(math::checked_div(
            math::checked_mul(position.size_usd as u128, Perpetuals::BPS_POWER)?,
            custody.pricing.max_leverage as u128,
        )?)?;
        let max_loss_usd = math::checked_add(max_loss_usd, unrealized_loss_usd)?;

        let margin_usd =
            math::checked_add(position.collateral_usd, position.unrealized_profit_usd)?;

        let max_price_diff = if max_loss_usd >= margin_usd {
            math::checked_sub(max_loss_usd, margin_usd)?
        } else {
            math::checked_sub(margin_usd, max_loss_usd)?
        };

        let position_price = math::scale_to_exponent(
            position.price,
            -(Perpetuals::PRICE_DECIMALS as i32),
            -(Perpetuals::USD_DECIMALS as i32),
        )?;

        let max_price_diff = math::checked_as_u64(math::checked_div(
            math::checked_mul(max_price_diff as u128, position_price as u128)?,
            position.size_usd as u128,
        )?)?;

        let max_price_diff = math::scale_to_exponent(
            max_price_diff,
            -(Perpetuals::USD_DECIMALS as i32),
            -(Perpetuals::PRICE_DECIMALS as i32),
        )?;

        if position.side == Side::Long {
            if max_loss_usd >= margin_usd {
                math::checked_add(position.price, max_price_diff)
            } else if position.price > max_price_diff {
                math::checked_sub(position.price, max_price_diff)
            } else {
                Ok(0)
            }
        } else if max_loss_usd >= margin_usd {
            if position.price > max_price_diff {
                math::checked_sub(position.price, max_price_diff)
            } else {
                Ok(0)
            }
        } else {
            math::checked_add(position.price, max_price_diff)
        }
    }

    // returns (profit_usd, loss_usd, fee_amount)
    #[allow(clippy::too_many_arguments)]
    pub fn get_pnl_usd(
        &self,
        position: &Position,
        token_price: &OraclePrice,
        token_ema_price: &OraclePrice,
        custody: &Custody,
        collateral_token_price: &OraclePrice,
        collateral_token_ema_price: &OraclePrice,
        collateral_custody: &Custody,
        curtime: i64,
        liquidation: bool,
    ) -> Result<(u64, u64, u64)> {
        if position.size_usd == 0 || position.price == 0 {
            return Ok((0, 0, 0));
        }

        let exit_price =
            self.get_exit_price(token_price, token_ema_price, position.side, custody)?;

        let size = token_ema_price.get_token_amount(position.size_usd, custody.decimals)?;

        let exit_fee = if liquidation {
            self.get_liquidation_fee(size, custody)?
        } else {
            self.get_exit_fee(size, custody)?
        };

        let exit_fee_usd = token_ema_price.get_asset_amount_usd(exit_fee, custody.decimals)?;
        let interest_usd = collateral_custody.get_interest_amount_usd(position, curtime)?;
        let unrealized_loss_usd = math::checked_add(
            math::checked_add(exit_fee_usd, interest_usd)?,
            position.unrealized_loss_usd,
        )?;

        let (price_diff_profit, price_diff_loss) = if position.side == Side::Long {
            if exit_price > position.price {
                (math::checked_sub(exit_price, position.price)?, 0u64)
            } else {
                (0u64, math::checked_sub(position.price, exit_price)?)
            }
        } else if exit_price < position.price {
            (math::checked_sub(position.price, exit_price)?, 0u64)
        } else {
            (0u64, math::checked_sub(exit_price, position.price)?)
        };

        let position_price = math::scale_to_exponent(
            position.price,
            -(Perpetuals::PRICE_DECIMALS as i32),
            -(Perpetuals::USD_DECIMALS as i32),
        )?;

        if price_diff_profit > 0 {
            let potential_profit_usd = math::checked_as_u64(math::checked_div(
                math::checked_mul(position.size_usd as u128, price_diff_profit as u128)?,
                position_price as u128,
            )?)?;

            let potential_profit_usd =
                math::checked_add(potential_profit_usd, position.unrealized_profit_usd)?;

            if potential_profit_usd >= unrealized_loss_usd {
                let cur_profit_usd = math::checked_sub(potential_profit_usd, unrealized_loss_usd)?;
                let min_collateral_price = if collateral_custody.is_virtual {
                    // if collateral_custody is virtual it means this function is called from get_assets_under_management_usd()
                    // (to calculate unrealized pnl of all open positions) and actual collateral custody is a stablecoin.
                    // we need to use 1USD reference price for such positions
                    OraclePrice {
                        price: 10u64.pow(Perpetuals::USD_DECIMALS as u32),
                        exponent: -(Perpetuals::USD_DECIMALS as i32),
                    }
                } else {
                    collateral_token_price
                        .get_min_price(collateral_token_ema_price, collateral_custody.is_stable)?
                };
                let max_profit_usd = if curtime <= position.open_time {
                    0
                } else {
                    min_collateral_price
                        .get_asset_amount_usd(position.locked_amount, collateral_custody.decimals)?
                };
                Ok((
                    std::cmp::min(max_profit_usd, cur_profit_usd),
                    0u64,
                    exit_fee,
                ))
            } else {
                Ok((
                    0u64,
                    math::checked_sub(unrealized_loss_usd, potential_profit_usd)?,
                    exit_fee,
                ))
            }
        } else {
            let potential_loss_usd = math::checked_as_u64(math::checked_ceil_div(
                math::checked_mul(position.size_usd as u128, price_diff_loss as u128)?,
                position_price as u128,
            )?)?;

            let potential_loss_usd = math::checked_add(potential_loss_usd, unrealized_loss_usd)?;

            if potential_loss_usd >= position.unrealized_profit_usd {
                Ok((
                    0u64,
                    math::checked_sub(potential_loss_usd, position.unrealized_profit_usd)?,
                    exit_fee,
                ))
            } else {
                let cur_profit_usd =
                    math::checked_sub(position.unrealized_profit_usd, potential_loss_usd)?;
                let min_collateral_price = if collateral_custody.is_virtual {
                    OraclePrice {
                        price: 10u64.pow(Perpetuals::USD_DECIMALS as u32),
                        exponent: -(Perpetuals::USD_DECIMALS as i32),
                    }
                } else {
                    collateral_token_price
                        .get_min_price(collateral_token_ema_price, collateral_custody.is_stable)?
                };
                let max_profit_usd = min_collateral_price
                    .get_asset_amount_usd(position.locked_amount, collateral_custody.decimals)?;
                Ok((
                    std::cmp::min(max_profit_usd, cur_profit_usd),
                    0u64,
                    exit_fee,
                ))
            }
        }
    }

    pub fn get_assets_under_management_usd(
        &self,
        aum_calc_mode: AumCalcMode,
        accounts: &[AccountInfo],
        curtime: i64,
    ) -> Result<u128> {
        let mut pool_amount_usd: u128 = 0;
        for (idx, &custody) in self.custodies.iter().enumerate() {
            let oracle_idx = idx + self.custodies.len();
            if oracle_idx >= accounts.len() {
                return Err(ProgramError::NotEnoughAccountKeys.into());
            }

            require_keys_eq!(accounts[idx].key(), custody);
            let custody = Account::<Custody>::try_from(&accounts[idx])?;

            require_keys_eq!(accounts[oracle_idx].key(), custody.oracle.oracle_account);

            let token_price = OraclePrice::new_from_oracle(
                &accounts[oracle_idx],
                &custody.oracle,
                curtime,
                false,
            )?;

            let token_ema_price = OraclePrice::new_from_oracle(
                &accounts[oracle_idx],
                &custody.oracle,
                curtime,
                custody.pricing.use_ema,
            )?;

            let aum_token_price = match aum_calc_mode {
                AumCalcMode::Last => token_price,
                AumCalcMode::EMA => token_ema_price,
                AumCalcMode::Min => {
                    if token_price < token_ema_price {
                        token_price
                    } else {
                        token_ema_price
                    }
                }
                AumCalcMode::Max => {
                    if token_price > token_ema_price {
                        token_price
                    } else {
                        token_ema_price
                    }
                }
            };

            let token_amount_usd =
                aum_token_price.get_asset_amount_usd(custody.assets.owned, custody.decimals)?;

            pool_amount_usd = math::checked_add(pool_amount_usd, token_amount_usd as u128)?;

            if custody.pricing.use_unrealized_pnl_in_aum {
                if custody.is_stable {
                    // compute accumulated interest
                    let collective_position = custody.get_collective_position(Side::Long)?;
                    let interest_usd =
                        custody.get_interest_amount_usd(&collective_position, curtime)?;
                    pool_amount_usd = math::checked_add(pool_amount_usd, interest_usd as u128)?;

                    let collective_position = custody.get_collective_position(Side::Short)?;
                    let interest_usd =
                        custody.get_interest_amount_usd(&collective_position, curtime)?;
                    pool_amount_usd = math::checked_add(pool_amount_usd, interest_usd as u128)?;
                } else {
                    // compute aggregate unrealized pnl
                    let (long_profit, long_loss, _) = self.get_pnl_usd(
                        &custody.get_collective_position(Side::Long)?,
                        &token_price,
                        &token_ema_price,
                        &custody,
                        &token_price,
                        &token_ema_price,
                        &custody,
                        curtime,
                        false,
                    )?;
                    let (short_profit, short_loss, _) = self.get_pnl_usd(
                        &custody.get_collective_position(Side::Short)?,
                        &token_price,
                        &token_ema_price,
                        &custody,
                        &token_price,
                        &token_ema_price,
                        &custody,
                        curtime,
                        false,
                    )?;

                    // adjust pool amount by collective profit/loss
                    pool_amount_usd = math::checked_add(pool_amount_usd, long_loss as u128)?;
                    pool_amount_usd = math::checked_add(pool_amount_usd, short_loss as u128)?;
                    pool_amount_usd = pool_amount_usd.saturating_sub(long_profit as u128);
                    pool_amount_usd = pool_amount_usd.saturating_sub(short_profit as u128);
                }
            }
        }

        Ok(pool_amount_usd)
    }

    pub fn get_fee_amount(fee: u64, amount: u64) -> Result<u64> {
        if fee == 0 || amount == 0 {
            return Ok(0);
        }
        math::checked_as_u64(math::checked_ceil_div(
            math::checked_mul(amount as u128, fee as u128)?,
            Perpetuals::BPS_POWER,
        )?)
    }

    // private helpers
    fn get_current_ratio(&self, custody: &Custody, token_price: &OraclePrice) -> Result<u64> {
        if self.aum_usd == 0 || custody.is_virtual {
            return Ok(0);
        }
        let ratio = math::checked_as_u64(math::checked_div(
            math::checked_mul(
                token_price.get_asset_amount_usd(custody.assets.owned, custody.decimals)? as u128,
                Perpetuals::BPS_POWER,
            )?,
            self.aum_usd,
        )?)?;
        Ok(std::cmp::min(ratio, Perpetuals::BPS_POWER as u64))
    }

    fn get_new_ratio(
        &self,
        amount_add: u64,
        amount_remove: u64,
        custody: &Custody,
        token_price: &OraclePrice,
    ) -> Result<u64> {
        if custody.is_virtual {
            return Ok(0);
        }
        let (new_token_aum_usd, new_pool_aum_usd) = if amount_add > 0 && amount_remove > 0 {
            return Err(ProgramError::InvalidArgument.into());
        } else if amount_add == 0 && amount_remove == 0 {
            (
                token_price.get_asset_amount_usd(custody.assets.owned, custody.decimals)? as u128,
                self.aum_usd,
            )
        } else if amount_add > 0 {
            let added_aum_usd =
                token_price.get_asset_amount_usd(amount_add, custody.decimals)? as u128;

            (
                token_price.get_asset_amount_usd(
                    math::checked_add(custody.assets.owned, amount_add)?,
                    custody.decimals,
                )? as u128,
                math::checked_add(self.aum_usd, added_aum_usd)?,
            )
        } else {
            let removed_aum_usd =
                token_price.get_asset_amount_usd(amount_remove, custody.decimals)? as u128;

            if removed_aum_usd >= self.aum_usd || amount_remove >= custody.assets.owned {
                (0, 0)
            } else {
                (
                    token_price.get_asset_amount_usd(
                        math::checked_sub(custody.assets.owned, amount_remove)?,
                        custody.decimals,
                    )? as u128,
                    math::checked_sub(self.aum_usd, removed_aum_usd)?,
                )
            }
        };
        if new_token_aum_usd == 0 || new_pool_aum_usd == 0 {
            return Ok(0);
        }

        let ratio = math::checked_as_u64(math::checked_div(
            math::checked_mul(new_token_aum_usd, Perpetuals::BPS_POWER)?,
            new_pool_aum_usd,
        )?)?;
        Ok(std::cmp::min(ratio, Perpetuals::BPS_POWER as u64))
    }

    fn get_price(
        &self,
        token_price: &OraclePrice,
        token_ema_price: &OraclePrice,
        side: Side,
        spread: u64,
    ) -> Result<OraclePrice> {
        if side == Side::Long {
            let max_price = if token_price > token_ema_price {
                token_price
            } else {
                token_ema_price
            };

            Ok(OraclePrice {
                price: math::checked_add(
                    max_price.price,
                    math::checked_decimal_ceil_mul(
                        max_price.price,
                        max_price.exponent,
                        spread,
                        -(Perpetuals::BPS_DECIMALS as i32),
                        max_price.exponent,
                    )?,
                )?,
                exponent: max_price.exponent,
            })
        } else {
            let min_price = if token_price < token_ema_price {
                token_price
            } else {
                token_ema_price
            };

            let spread = math::checked_decimal_mul(
                min_price.price,
                min_price.exponent,
                spread,
                -(Perpetuals::BPS_DECIMALS as i32),
                min_price.exponent,
            )?;

            let price = if spread < min_price.price {
                math::checked_sub(min_price.price, spread)?
            } else {
                0
            };

            Ok(OraclePrice {
                price,
                exponent: min_price.exponent,
            })
        }
    }

    fn get_fee(
        &self,
        token_id: usize,
        base_fee: u64,
        amount_add: u64,
        amount_remove: u64,
        custody: &Custody,
        token_price: &OraclePrice,
    ) -> Result<u64> {
        require!(!custody.is_virtual, PerpetualsError::InstructionNotAllowed);

        if custody.fees.mode == FeesMode::Fixed {
            return Self::get_fee_amount(base_fee, std::cmp::max(amount_add, amount_remove));
        }

        // if token ratio is improved:
        //    fee = base_fee / ratio_fee
        // otherwise:
        //    fee = base_fee * ratio_fee
        // where:
        //   if new_ratio < ratios.target:
        //     ratio_fee = 1 + custody.fees.ratio_mult * (ratios.target - new_ratio) / (ratios.target - ratios.min);
        //   otherwise:
        //     ratio_fee = 1 + custody.fees.ratio_mult * (new_ratio - ratios.target) / (ratios.max - ratios.target);

        let ratios = &self.ratios[token_id];
        let current_ratio = self.get_current_ratio(custody, token_price)?;
        let new_ratio = self.get_new_ratio(amount_add, amount_remove, custody, token_price)?;

        let improved = match new_ratio.cmp(&ratios.target) {
            Ordering::Less => {
                new_ratio > current_ratio
                    || (current_ratio > ratios.target
                        && current_ratio - ratios.target > ratios.target - new_ratio)
            }
            Ordering::Greater => {
                new_ratio < current_ratio
                    || (current_ratio < ratios.target
                        && ratios.target - current_ratio > new_ratio - ratios.target)
            }
            Ordering::Equal => current_ratio != ratios.target,
        };

        let ratio_fee = if new_ratio <= ratios.target {
            if ratios.target == ratios.min {
                Perpetuals::BPS_POWER
            } else {
                math::checked_add(
                    Perpetuals::BPS_POWER,
                    math::checked_div(
                        math::checked_mul(
                            custody.fees.ratio_mult as u128,
                            math::checked_sub(ratios.target, new_ratio)? as u128,
                        )?,
                        math::checked_sub(ratios.target, ratios.min)? as u128,
                    )?,
                )?
            }
        } else if ratios.target == ratios.max {
            Perpetuals::BPS_POWER
        } else {
            math::checked_add(
                Perpetuals::BPS_POWER,
                math::checked_div(
                    math::checked_mul(
                        custody.fees.ratio_mult as u128,
                        math::checked_sub(new_ratio, ratios.target)? as u128,
                    )?,
                    math::checked_sub(ratios.max, ratios.target)? as u128,
                )?,
            )?
        };

        let fee = if improved {
            math::checked_div(
                math::checked_mul(base_fee as u128, Perpetuals::BPS_POWER)?,
                ratio_fee,
            )?
        } else {
            math::checked_div(
                math::checked_mul(base_fee as u128, ratio_fee)?,
                Perpetuals::BPS_POWER,
            )?
        };

        Self::get_fee_amount(
            math::checked_as_u64(fee)?,
            std::cmp::max(amount_add, amount_remove),
        )
    }
}

#[cfg(test)]
mod test {
    use {
        super::*,
        crate::state::{
            custody::{BorrowRateParams, Fees, PricingParams},
            oracle::{OracleParams, OracleType},
            perpetuals::Permissions,
        },
    };

    fn get_fixture() -> (Pool, Custody, Position, OraclePrice, OraclePrice) {
        let ratios = TokenRatios {
            target: 5_000,
            min: 1_000,
            max: 9_000,
        };

        let oracle = OracleParams {
            oracle_account: Pubkey::default(),
            oracle_type: OracleType::Custom,
            max_price_error: 100,
            max_price_age_sec: 1,
        };

        let pricing = PricingParams {
            use_ema: true,
            use_unrealized_pnl_in_aum: true,
            trade_spread_long: 100,
            trade_spread_short: 100,
            swap_spread: 300,
            min_initial_leverage: 10_000,
            max_initial_leverage: 100_000,
            max_leverage: 100_000,
            max_payoff_mult: 10_000,
            max_utilization: 0,
            max_position_locked_usd: 0,
            max_total_locked_usd: 0,
        };

        let permissions = Permissions {
            allow_swap: true,
            allow_add_liquidity: true,
            allow_remove_liquidity: true,
            allow_open_position: true,
            allow_close_position: true,
            allow_pnl_withdrawal: true,
            allow_collateral_withdrawal: true,
            allow_size_change: true,
        };

        let fees = Fees {
            mode: FeesMode::Linear,
            ratio_mult: 20_000,
            utilization_mult: 20_000,
            swap_in: 100,
            swap_out: 100,
            stable_swap_in: 100,
            stable_swap_out: 100,
            add_liquidity: 200,
            remove_liquidity: 300,
            open_position: 100,
            close_position: 0,
            liquidation: 50,
            protocol_share: 25,
        };

        let custody = Custody {
            token_account: Pubkey::default(),
            mint: Pubkey::default(),
            decimals: 9,
            oracle,
            pricing,
            permissions,
            fees,
            ..Custody::default()
        };

        let position = Position {
            side: Side::Long,
            price: scale(25_000, Perpetuals::PRICE_DECIMALS),
            // x4 leverage
            size_usd: scale(100_000, Perpetuals::USD_DECIMALS),
            collateral_usd: scale(25_000, Perpetuals::USD_DECIMALS),
            locked_amount: scale(4, 9),
            collateral_amount: scale(1, 9),
            ..Position::default()
        };

        let token_price = OraclePrice {
            price: 25_000_000,
            exponent: -3,
        };
        let token_ema_price = OraclePrice {
            price: 25_300_000,
            exponent: -3,
        };

        (
            Pool {
                name: "Test Pool".to_string(),
                ratios: vec![ratios, ratios],
                ..Default::default()
            },
            custody,
            position,
            token_price,
            token_ema_price,
        )
    }

    fn scale(amount: u64, decimals: u8) -> u64 {
        math::checked_mul(amount, 10u64.pow(decimals as u32)).unwrap()
    }

    fn scale_f64(amount: f64, decimals: u8) -> u64 {
        math::checked_as_u64(
            math::checked_float_mul(amount, 10u64.pow(decimals as u32) as f64).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn test_get_new_ratio() {
        let (mut pool, mut custody, _position, token_price, _token_ema_price) = get_fixture();

        // add tokens to empty custody
        assert_eq!(
            scale(1, Perpetuals::BPS_DECIMALS),
            pool.get_new_ratio(1_000, 0, &custody, &token_price)
                .unwrap()
        );

        // remove tokens from empty custody
        assert_eq!(
            0,
            pool.get_new_ratio(0, 1_000, &custody, &token_price)
                .unwrap()
        );

        // cannot provide both add and remove
        assert!(pool
            .get_new_ratio(1_000, 1_000, &custody, &token_price)
            .is_err());

        // doesn't change ratio
        assert_eq!(0, pool.get_new_ratio(0, 0, &custody, &token_price).unwrap());

        // add value to the pool for the custody to be 50% ratio
        pool.aum_usd = scale(50_000_000, Perpetuals::USD_DECIMALS) as u128;
        custody.assets.owned = scale(1_000, custody.decimals);

        // add tokens to go 50%+ ratio
        assert_eq!(
            5238,
            pool.get_new_ratio(scale(100, custody.decimals), 0, &custody, &token_price)
                .unwrap()
        );

        // removes tokens to get 50%- ratio
        assert_eq!(
            4_736,
            pool.get_new_ratio(0, scale(100, custody.decimals), &custody, &token_price)
                .unwrap()
        );

        // removes all tokens to get to 0% ratio
        assert_eq!(
            0,
            pool.get_new_ratio(0, scale(1_000, custody.decimals), &custody, &token_price)
                .unwrap()
        );

        // changes nothing should return same ratio
        assert_eq!(
            5_000,
            pool.get_new_ratio(0, 0, &custody, &token_price).unwrap()
        );
    }

    #[test]
    fn test_get_price() {
        let (pool, custody, _position, token_price, token_ema_price) = get_fixture();

        assert_eq!(
            OraclePrice {
                price: 25_553_000,
                exponent: -3
            },
            pool.get_price(
                &token_price,
                &token_ema_price,
                Side::Long,
                custody.pricing.trade_spread_long,
            )
            .unwrap()
        );

        assert_eq!(
            OraclePrice {
                price: 24_750_000,
                exponent: -3
            },
            pool.get_price(
                &token_price,
                &token_ema_price,
                Side::Short,
                custody.pricing.trade_spread_short,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_get_entry_fee() {
        let (pool, mut custody, _position, _token_price, _token_ema_price) = get_fixture();

        custody.fees.utilization_mult = 20_000;
        custody.assets.owned = 200_000;
        custody.borrow_rate.optimal_utilization = 500_000_000;

        assert_eq!(
            0,
            pool.get_entry_fee(
                custody.fees.open_position,
                0,
                custody.get_locked_amount(0, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );

        assert_eq!(
            1_000,
            pool.get_entry_fee(
                custody.fees.open_position,
                100_000,
                custody.get_locked_amount(100_000, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );

        assert_eq!(
            3_000,
            pool.get_entry_fee(
                custody.fees.open_position,
                150_000,
                custody.get_locked_amount(150_000, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );

        assert_eq!(
            6_000,
            pool.get_entry_fee(
                custody.fees.open_position,
                200_000,
                custody.get_locked_amount(200_000, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );

        assert_eq!(
            9_000,
            pool.get_entry_fee(
                custody.fees.open_position,
                300_000,
                custody.get_locked_amount(300_000, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );

        custody.fees.utilization_mult = 10_000;
        custody.assets.owned = 200_000;
        custody.borrow_rate.optimal_utilization = 500_000_000;

        assert_eq!(
            1_000,
            pool.get_entry_fee(
                custody.fees.open_position,
                100_000,
                custody.get_locked_amount(100_000, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );

        assert_eq!(
            2_250,
            pool.get_entry_fee(
                custody.fees.open_position,
                150_000,
                custody.get_locked_amount(150_000, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );

        assert_eq!(
            4_000,
            pool.get_entry_fee(
                custody.fees.open_position,
                200_000,
                custody.get_locked_amount(200_000, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );

        assert_eq!(
            6_000,
            pool.get_entry_fee(
                custody.fees.open_position,
                300_000,
                custody.get_locked_amount(300_000, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );

        custody.fees.utilization_mult = 5_000;

        assert_eq!(
            1_000,
            pool.get_entry_fee(
                custody.fees.open_position,
                100_000,
                custody.get_locked_amount(100_000, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );

        assert_eq!(
            1_875,
            pool.get_entry_fee(
                custody.fees.open_position,
                150_000,
                custody.get_locked_amount(150_000, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );

        assert_eq!(
            3_000,
            pool.get_entry_fee(
                custody.fees.open_position,
                200_000,
                custody.get_locked_amount(200_000, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );

        assert_eq!(
            4_500,
            pool.get_entry_fee(
                custody.fees.open_position,
                300_000,
                custody.get_locked_amount(300_000, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );

        custody.fees.utilization_mult = 20_000;
        custody.borrow_rate.optimal_utilization = 1_000_000_000;

        assert_eq!(
            1_000,
            pool.get_entry_fee(
                custody.fees.open_position,
                100_000,
                custody.get_locked_amount(100_000, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );

        assert_eq!(
            1_500,
            pool.get_entry_fee(
                custody.fees.open_position,
                150_000,
                custody.get_locked_amount(150_000, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );

        assert_eq!(
            2_000,
            pool.get_entry_fee(
                custody.fees.open_position,
                200_000,
                custody.get_locked_amount(200_000, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );

        assert_eq!(
            3_000,
            pool.get_entry_fee(
                custody.fees.open_position,
                300_000,
                custody.get_locked_amount(300_000, Side::Long).unwrap(),
                &custody
            )
            .unwrap()
        );
    }

    #[test]
    fn test_get_fee() {
        let (mut pool, mut custody, _position, token_price, _token_ema_price) = get_fixture();

        custody.fees.mode = FeesMode::Fixed;
        assert_eq!(
            scale_f64(0.2, custody.decimals),
            pool.get_fee(
                0,
                custody.fees.swap_in,
                scale(20, custody.decimals),
                0,
                &custody,
                &token_price
            )
            .unwrap()
        );

        custody.fees.mode = FeesMode::Linear;

        custody.assets.owned = scale(15, custody.decimals); // $375,000
        pool.ratios[0].min = 1_000;
        pool.ratios[0].target = 5_000;
        pool.ratios[0].max = 9_000;

        // makes custody ratio higher than target
        pool.aum_usd = scale(600_000, Perpetuals::USD_DECIMALS) as u128;

        // move further away from target
        assert_eq!(
            97_000_000,
            pool.get_fee(
                0,
                custody.fees.swap_in,
                scale(5, custody.decimals),
                0,
                &custody,
                &token_price,
            )
            .unwrap()
        );

        // move toward target
        assert_eq!(
            13_600_000,
            pool.get_fee(
                0,
                custody.fees.swap_in,
                0,
                scale(2, custody.decimals),
                &custody,
                &token_price,
            )
            .unwrap()
        );

        // match target
        assert_eq!(
            60_000_000,
            pool.get_fee(
                0,
                custody.fees.swap_in,
                0,
                scale(6, custody.decimals),
                &custody,
                &token_price,
            )
            .unwrap()
        );

        // makes custody ratio lower than target
        pool.aum_usd = scale(1_200_000, Perpetuals::USD_DECIMALS) as u128;

        // move toward target
        assert_eq!(
            30_500_000,
            pool.get_fee(
                0,
                custody.fees.swap_in,
                scale(5, custody.decimals),
                0,
                &custody,
                &token_price,
            )
            .unwrap()
        );

        // move away from target
        assert_eq!(
            116_500_000,
            pool.get_fee(
                0,
                custody.fees.swap_in,
                0,
                scale(5, custody.decimals),
                &custody,
                &token_price,
            )
            .unwrap()
        );

        // match target
        assert_eq!(
            180_000_000,
            pool.get_fee(
                0,
                custody.fees.swap_in,
                scale(18, custody.decimals),
                0,
                &custody,
                &token_price,
            )
            .unwrap()
        );
    }

    #[test]
    fn test_get_pnl_usd() {
        let (pool, custody, mut position, token_price, token_ema_price) = get_fixture();

        // initial PnL at loss
        assert_eq!(
            (0, scale(1_000, Perpetuals::USD_DECIMALS), 0),
            pool.get_pnl_usd(
                &position,
                &token_price,
                &token_ema_price,
                &custody,
                &token_price,
                &token_ema_price,
                &custody,
                0,
                false
            )
            .unwrap()
        );

        // losing position (opening price higher than current price)
        position.price = scale(25_400, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            (0, scale_f64(2_559.055119, Perpetuals::USD_DECIMALS), 0),
            pool.get_pnl_usd(
                &position,
                &token_price,
                &token_ema_price,
                &custody,
                &token_price,
                &token_ema_price,
                &custody,
                0,
                false
            )
            .unwrap()
        );

        // winning position (opening price lower than current price)
        position.price = scale(24_500, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            (scale_f64(1_020.408163, Perpetuals::USD_DECIMALS), 0, 0),
            pool.get_pnl_usd(
                &position,
                &token_price,
                &token_ema_price,
                &custody,
                &token_price,
                &token_ema_price,
                &custody,
                0,
                false
            )
            .unwrap()
        );
    }

    #[test]
    fn test_get_leverage() {
        let (pool, custody, mut position, token_price, token_ema_price) = get_fixture();

        // default leverage
        assert_eq!(
            scale_f64(4.1666, Perpetuals::BPS_DECIMALS),
            pool.get_leverage(
                &position,
                &token_price,
                &token_ema_price,
                &custody,
                &token_price,
                &token_ema_price,
                &custody,
                0
            )
            .unwrap()
        );

        // lower price should lower leverage for long position
        position.price = scale(20_000, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale_f64(2.0512, Perpetuals::BPS_DECIMALS),
            pool.get_leverage(
                &position,
                &token_price,
                &token_ema_price,
                &custody,
                &token_price,
                &token_ema_price,
                &custody,
                0
            )
            .unwrap()
        );

        position.price = scale(15_000, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale_f64(1.1111, Perpetuals::BPS_DECIMALS),
            pool.get_leverage(
                &position,
                &token_price,
                &token_ema_price,
                &custody,
                &token_price,
                &token_ema_price,
                &custody,
                0
            )
            .unwrap()
        );

        // higher price should increase leverage for long position
        position.price = scale(27_000, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale_f64(6.0000, Perpetuals::BPS_DECIMALS),
            pool.get_leverage(
                &position,
                &token_price,
                &token_ema_price,
                &custody,
                &token_price,
                &token_ema_price,
                &custody,
                0
            )
            .unwrap()
        );

        position.price = scale(32_000, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale_f64(42.6666, Perpetuals::BPS_DECIMALS),
            pool.get_leverage(
                &position,
                &token_price,
                &token_ema_price,
                &custody,
                &token_price,
                &token_ema_price,
                &custody,
                0
            )
            .unwrap()
        );

        // no price should return raw leverage
        position.price = scale(0, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale(4, Perpetuals::BPS_DECIMALS),
            pool.get_leverage(
                &position,
                &token_price,
                &token_ema_price,
                &custody,
                &token_price,
                &token_ema_price,
                &custody,
                0
            )
            .unwrap()
        );

        // leverage out of limit
        position.price = scale(40_000, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            u64::MAX,
            pool.get_leverage(
                &position,
                &token_price,
                &token_ema_price,
                &custody,
                &token_price,
                &token_ema_price,
                &custody,
                0
            )
            .unwrap()
        );
    }

    #[test]
    fn test_get_liquidation_price() {
        let (pool, custody, mut position, token_price, _token_ema_price) = get_fixture();

        assert_eq!(
            scale(21_250, Perpetuals::PRICE_DECIMALS),
            pool.get_liquidation_price(&position, &token_price, &custody, &custody, 0)
                .unwrap()
        );

        // lower price should lower liquidation price
        position.price = scale(24_500, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale(20_825, Perpetuals::PRICE_DECIMALS),
            pool.get_liquidation_price(&position, &token_price, &custody, &custody, 0)
                .unwrap()
        );

        position.price = scale(20_000, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale(17_000, Perpetuals::PRICE_DECIMALS),
            pool.get_liquidation_price(&position, &token_price, &custody, &custody, 0)
                .unwrap()
        );

        // higher price should increase liquidation price
        position.price = scale(26_000, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale(22_100, Perpetuals::PRICE_DECIMALS),
            pool.get_liquidation_price(&position, &token_price, &custody, &custody, 0)
                .unwrap()
        );

        position.price = scale(35_000, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale(29_750, Perpetuals::PRICE_DECIMALS),
            pool.get_liquidation_price(&position, &token_price, &custody, &custody, 0)
                .unwrap()
        );

        // dead price
        position.price = scale(0, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale_f64(0.0, Perpetuals::PRICE_DECIMALS),
            pool.get_liquidation_price(&position, &token_price, &custody, &custody, 0)
                .unwrap()
        );
    }

    #[test]
    fn test_get_close_amount() {
        let (pool, custody, position, token_price, token_ema_price) = get_fixture();

        assert_eq!(
            (
                scale_f64(0.948616600, custody.decimals),
                0,
                0,
                scale(1_000, Perpetuals::USD_DECIMALS)
            ),
            pool.get_close_amount(
                &position,
                &token_price,
                &token_ema_price,
                &custody,
                &token_price,
                &token_ema_price,
                &custody,
                0,
                false
            )
            .unwrap()
        );
    }

    #[test]
    fn test_get_interest_amount_usd() {
        let (_pool, mut custody, mut position, _token_price, _token_ema_price) = get_fixture();

        custody.borrow_rate = BorrowRateParams {
            base_rate: 0,
            slope1: 80_000,
            slope2: 120_000,
            optimal_utilization: 800_000_000,
        };
        custody.assets.locked = scale(9, 9);
        custody.assets.owned = scale(10, 9);

        custody.update_borrow_rate(3_600).unwrap();
        let interest = custody.get_interest_amount_usd(&position, 3_600).unwrap();
        assert_eq!(interest, 0);

        let interest = custody.get_interest_amount_usd(&position, 7_200).unwrap();
        assert_eq!(interest, scale(14, Perpetuals::USD_DECIMALS));

        custody.update_borrow_rate(7_200).unwrap();
        let interest = custody.get_interest_amount_usd(&position, 7_199).unwrap();
        assert_eq!(interest, scale(14, Perpetuals::USD_DECIMALS));

        position.cumulative_interest_snapshot = 70_000;
        let interest = custody.get_interest_amount_usd(&position, 7_200).unwrap();
        assert_eq!(interest, scale(7, Perpetuals::USD_DECIMALS));
    }
}
