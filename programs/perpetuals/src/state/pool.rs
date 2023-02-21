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

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct PoolToken {
    pub custody: Pubkey,

    // ratios have implied BPS_DECIMALS decimals
    pub target_ratio: u64,
    pub min_ratio: u64,
    pub max_ratio: u64,
}

#[account]
#[derive(Default, Debug)]
pub struct Pool {
    pub name: String,
    pub tokens: Vec<PoolToken>,
    pub aum_usd: u128,

    pub bump: u8,
    pub lp_token_bump: u8,
    pub inception_time: i64,
}

/// Token Pool
/// All returned prices are scaled to PRICE_DECIMALS.
/// All returned amounts are scaled to corresponding custody decimals.
///
impl Pool {
    pub const LEN: usize = 8 + std::mem::size_of::<Pool>();

    pub fn get_token_id(&self, custody: &Pubkey) -> Result<usize> {
        self.tokens
            .iter()
            .position(|&k| k.custody == *custody)
            .ok_or(PerpetualsError::UnsupportedToken.into())
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

        Ok(price
            .scale_to_exponent(-(Perpetuals::PRICE_DECIMALS as i32))?
            .price)
    }

    pub fn get_entry_fee(
        &self,
        token_id: usize,
        collateral: u64,
        size: u64,
        custody: &Custody,
        token_price: &OraclePrice,
    ) -> Result<u64> {
        let collateral_fee =
            self.get_add_liquidity_fee(token_id, collateral, custody, token_price)?;
        let size_fee = Self::get_fee_amount(custody.fees.open_position, size)?;

        math::checked_add(collateral_fee, size_fee)
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

    pub fn get_exit_fee(
        &self,
        token_id: usize,
        collateral: u64,
        custody: &Custody,
        token_price: &OraclePrice,
    ) -> Result<u64> {
        self.get_remove_liquidity_fee(token_id, collateral, custody, token_price)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_close_amount(
        &self,
        token_id: usize,
        position: &Position,
        token_price: &OraclePrice,
        token_ema_price: &OraclePrice,
        custody: &Custody,
        size_usd: u64,
        curtime: i64,
        liquidation: bool,
    ) -> Result<(u64, u64, u64, u64)> {
        let (profit_usd, loss_usd, fee_amount) = self.get_pnl_usd(
            token_id,
            position,
            token_price,
            token_ema_price,
            custody,
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

        let close_amount_usd = if size_usd < position.size_usd {
            math::checked_as_u64(math::checked_div(
                math::checked_mul(available_amount_usd as u128, size_usd as u128)?,
                position.size_usd as u128,
            )?)?
        } else {
            available_amount_usd
        };

        let close_amount = token_price.get_token_amount(close_amount_usd, custody.decimals)?;
        let max_amount = math::checked_add(position.locked_amount, position.collateral_amount)?;

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
        let add_liquidity_fee =
            self.get_add_liquidity_fee(token_id_in, amount_in, custody_in, token_price_in)?;

        let remove_liquidity_fee =
            self.get_remove_liquidity_fee(token_id_out, amount_out, custody_out, token_price_out)?;

        let swap_fee = Self::get_fee_amount(custody_out.fees.swap, amount_out)?;

        Ok((
            add_liquidity_fee,
            math::checked_add(remove_liquidity_fee, swap_fee)?,
        ))
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

    pub fn get_liquidation_fee(
        &self,
        token_id: usize,
        amount: u64,
        custody: &Custody,
        token_price: &OraclePrice,
    ) -> Result<u64> {
        self.get_fee(
            token_id,
            custody.fees.liquidation,
            0u64,
            amount,
            custody,
            token_price,
        )
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

        Ok(new_ratio <= self.tokens[token_id].max_ratio
            && new_ratio >= self.tokens[token_id].min_ratio)
    }

    pub fn check_available_amount(&self, amount: u64, custody: &Custody) -> Result<bool> {
        let available_amount = math::checked_sub(
            math::checked_add(custody.assets.owned, custody.assets.collateral)?,
            custody.assets.locked,
        )?;
        Ok(available_amount >= amount)
    }

    pub fn get_interest_amount_usd(
        &self,
        position: &Position,
        custody: &Custody,
        curtime: i64,
    ) -> Result<u64> {
        let cumulative_interest = custody.get_cumulative_interest(curtime)?;

        let position_interest = if cumulative_interest > position.cumulative_interest_snapshot {
            math::checked_sub(cumulative_interest, position.cumulative_interest_snapshot)?
        } else {
            return Ok(0);
        };

        math::checked_as_u64(math::checked_div(
            math::checked_mul(position_interest, position.size_usd as u128)?,
            Perpetuals::RATE_POWER,
        )?)
    }

    pub fn get_leverage(
        &self,
        token_id: usize,
        position: &Position,
        token_price: &OraclePrice,
        token_ema_price: &OraclePrice,
        custody: &Custody,
        curtime: i64,
    ) -> Result<u64> {
        let (profit_usd, loss_usd, _) = self.get_pnl_usd(
            token_id,
            position,
            token_price,
            token_ema_price,
            custody,
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

    pub fn get_initial_leverage(&self, position: &Position) -> Result<u64> {
        math::checked_as_u64(math::checked_div(
            math::checked_mul(position.size_usd as u128, Perpetuals::BPS_POWER)?,
            position.collateral_usd as u128,
        )?)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn check_leverage(
        &self,
        token_id: usize,
        position: &Position,
        token_price: &OraclePrice,
        token_ema_price: &OraclePrice,
        custody: &Custody,
        curtime: i64,
        initial: bool,
    ) -> Result<bool> {
        let current_leverage = self.get_leverage(
            token_id,
            position,
            token_price,
            token_ema_price,
            custody,
            curtime,
        )?;

        Ok(current_leverage <= custody.pricing.max_leverage
            && (!initial || current_leverage >= custody.pricing.min_initial_leverage))
    }

    pub fn get_liquidation_price(
        &self,
        token_id: usize,
        position: &Position,
        token_price: &OraclePrice,
        custody: &Custody,
    ) -> Result<u64> {
        // liq_price_long = pos_price - (collateral + unreal_profit - (exit_fee + unreal_loss + size / max_leverage)) / init_leverage + spread
        // liq_price_short = pos_price + (collateral + unreal_unprofit - (exit_fee + unreal_loss + size / max_leverage)) / init_leverage - spread
        let collateral = token_price.get_token_amount(position.collateral_usd, custody.decimals)?;

        let exit_fee_tokens = self.get_exit_fee(token_id, collateral, custody, token_price)?;

        let exit_fee_usd = token_price.get_asset_amount_usd(exit_fee_tokens, custody.decimals)?;

        let max_loss_usd = math::checked_as_u64(math::checked_add(
            math::checked_div(
                math::checked_mul(position.size_usd as u128, Perpetuals::BPS_POWER)?,
                custody.pricing.max_leverage as u128,
            )?,
            exit_fee_usd as u128,
        )?)?;

        let initial_leverage = self.get_initial_leverage(position)?;

        let max_price_diff = if max_loss_usd >= position.collateral_usd {
            math::checked_sub(max_loss_usd, position.collateral_usd)?
        } else {
            math::checked_sub(position.collateral_usd, max_loss_usd)?
        };

        let max_price_diff = math::checked_as_u64(math::checked_div(
            math::checked_mul(max_price_diff as u128, Perpetuals::BPS_POWER)?,
            initial_leverage as u128,
        )?)?;

        let max_price_diff = math::scale_to_exponent(
            max_price_diff,
            -(Perpetuals::USD_DECIMALS as i32),
            -(Perpetuals::PRICE_DECIMALS as i32),
        )?;

        let price_no_spread = if position.side == Side::Long {
            if max_loss_usd >= position.collateral_usd {
                math::checked_add(position.price, max_price_diff)?
            } else if position.price > max_price_diff {
                math::checked_sub(position.price, max_price_diff)?
            } else {
                0
            }
        } else if max_loss_usd >= position.collateral_usd {
            if position.price > max_price_diff {
                math::checked_sub(position.price, max_price_diff)?
            } else {
                0
            }
        } else {
            math::checked_add(position.price, max_price_diff)?
        };

        let oracle_price_no_spread = OraclePrice {
            price: price_no_spread,
            exponent: -(Perpetuals::PRICE_DECIMALS as i32),
        };

        Ok(self
            .get_price(
                &oracle_price_no_spread,
                &oracle_price_no_spread,
                position.side,
                if position.side == Side::Long {
                    custody.pricing.trade_spread_short
                } else {
                    custody.pricing.trade_spread_long
                },
            )?
            .price)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_pnl_usd(
        &self,
        token_id: usize,
        position: &Position,
        token_price: &OraclePrice,
        token_ema_price: &OraclePrice,
        custody: &Custody,
        curtime: i64,
        liquidation: bool,
    ) -> Result<(u64, u64, u64)> {
        let collateral = token_price.get_token_amount(position.collateral_usd, custody.decimals)?;

        let exit_price =
            self.get_exit_price(token_price, token_ema_price, position.side, custody)?;

        let exit_fee = if liquidation {
            self.get_liquidation_fee(token_id, collateral, custody, token_price)?
        } else {
            self.get_exit_fee(token_id, collateral, custody, token_price)?
        };

        let exit_fee_usd = token_price.get_asset_amount_usd(exit_fee, custody.decimals)?;
        let interest_usd = self.get_interest_amount_usd(position, custody, curtime)?;
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

        let position_leverage = self.get_initial_leverage(position)?;

        if price_diff_profit > 0 {
            let potential_profit_usd = math::checked_decimal_mul(
                price_diff_profit,
                -(Perpetuals::PRICE_DECIMALS as i32),
                position_leverage,
                -(Perpetuals::BPS_DECIMALS as i32),
                -(Perpetuals::USD_DECIMALS as i32),
            )?;

            let potential_profit_usd =
                math::checked_add(potential_profit_usd, position.unrealized_profit_usd)?;

            if potential_profit_usd >= unrealized_loss_usd {
                let cur_profit_usd = math::checked_sub(potential_profit_usd, unrealized_loss_usd)?;
                let max_profit_usd =
                    token_price.get_asset_amount_usd(position.locked_amount, custody.decimals)?;
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
            let potential_loss_usd = math::checked_decimal_mul(
                price_diff_loss,
                -(Perpetuals::PRICE_DECIMALS as i32),
                position_leverage,
                -(Perpetuals::BPS_DECIMALS as i32),
                -(Perpetuals::USD_DECIMALS as i32),
            )?;

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
                let max_profit_usd =
                    token_price.get_asset_amount_usd(position.locked_amount, custody.decimals)?;
                Ok((
                    std::cmp::min(max_profit_usd, cur_profit_usd),
                    0u64,
                    exit_fee,
                ))
            }
        }
    }

    pub fn lock_funds(&self, amount: u64, custody: &mut Custody) -> Result<()> {
        custody.assets.locked = math::checked_add(custody.assets.locked, amount)?;

        if custody.assets.owned < custody.assets.locked {
            Err(ProgramError::InsufficientFunds.into())
        } else {
            Ok(())
        }
    }

    pub fn unlock_funds(&self, amount: u64, custody: &mut Custody) -> Result<()> {
        if amount > custody.assets.locked {
            custody.assets.locked = 0;
        } else {
            custody.assets.locked = math::checked_sub(custody.assets.locked, amount)?;
        }

        Ok(())
    }

    pub fn get_assets_under_management_usd(
        &self,
        accounts: &[AccountInfo],
        curtime: i64,
    ) -> Result<u128> {
        let mut pool_amount_usd: u128 = 0;
        for (idx, &token) in self.tokens.iter().enumerate() {
            let oracle_idx = idx + self.tokens.len();
            if oracle_idx >= accounts.len() {
                return Err(ProgramError::NotEnoughAccountKeys.into());
            }

            require_keys_eq!(accounts[idx].key(), token.custody);
            let custody = Account::<Custody>::try_from(&accounts[idx])?;
            require_keys_eq!(accounts[oracle_idx].key(), custody.oracle.oracle_account);

            let token_price = OraclePrice::new_from_oracle(
                custody.oracle.oracle_type,
                &accounts[oracle_idx],
                custody.oracle.max_price_error,
                custody.oracle.max_price_age_sec,
                curtime,
            )?;

            let token_amount_usd =
                token_price.get_asset_amount_usd(custody.assets.owned, custody.decimals)?;

            pool_amount_usd = math::checked_add(pool_amount_usd, token_amount_usd as u128)?;
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
    fn get_new_ratio(
        &self,
        amount_add: u64,
        amount_remove: u64,
        custody: &Custody,
        token_price: &OraclePrice,
    ) -> Result<u64> {
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

        math::checked_as_u64(math::checked_div(
            math::checked_mul(new_token_aum_usd, Perpetuals::BPS_POWER)?,
            new_pool_aum_usd,
        )?)
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
        if custody.fees.mode == FeesMode::Fixed {
            return Self::get_fee_amount(base_fee, std::cmp::max(amount_add, amount_remove));
        }
        let token = &self.tokens[token_id];
        let new_ratio = self.get_new_ratio(amount_add, amount_remove, custody, token_price)?;

        let fee = match new_ratio.cmp(&token.target_ratio) {
            Ordering::Equal => custody.fees.open_position,
            Ordering::Greater => {
                let max_fee_change = math::checked_as_u64(math::checked_div(
                    math::checked_mul(
                        custody.fees.max_increase as u128,
                        custody.fees.open_position as u128,
                    )?,
                    Perpetuals::BPS_POWER,
                )?)?;

                if token.max_ratio <= token.target_ratio || token.max_ratio <= new_ratio {
                    math::checked_add(custody.fees.open_position, max_fee_change)?
                } else {
                    math::checked_add(
                        custody.fees.open_position,
                        math::checked_as_u64(math::checked_ceil_div(
                            math::checked_mul(
                                math::checked_sub(
                                    std::cmp::min(token.max_ratio, new_ratio),
                                    token.target_ratio,
                                )? as u128,
                                max_fee_change as u128,
                            )?,
                            math::checked_sub(token.max_ratio, token.target_ratio)? as u128,
                        )?)?,
                    )?
                }
            }
            Ordering::Less => {
                let max_fee_change = math::checked_as_u64(math::checked_div(
                    math::checked_mul(
                        custody.fees.max_decrease as u128,
                        custody.fees.open_position as u128,
                    )?,
                    Perpetuals::BPS_POWER,
                )?)?;

                if token.target_ratio <= token.min_ratio || token.max_ratio <= new_ratio {
                    math::checked_sub(custody.fees.open_position, max_fee_change)?
                } else {
                    let fee_reduce = math::checked_as_u64(math::checked_ceil_div(
                        math::checked_mul(
                            math::checked_sub(
                                token.target_ratio,
                                std::cmp::max(token.min_ratio, new_ratio),
                            )? as u128,
                            max_fee_change as u128,
                        )?,
                        math::checked_sub(token.target_ratio, token.min_ratio)? as u128,
                    )?)?;
                    if custody.fees.open_position > fee_reduce {
                        math::checked_sub(custody.fees.open_position, fee_reduce)?
                    } else {
                        0
                    }
                }
            }
        };

        Self::get_fee_amount(fee, std::cmp::max(amount_add, amount_remove))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::state::{
        custody::{BorrowRateParams, Fees, OracleParams, PricingParams},
        oracle::OracleType,
        perpetuals::Permissions,
    };

    fn get_fixture() -> (Pool, Custody, Position, OraclePrice, OraclePrice) {
        let token = PoolToken {
            custody: Pubkey::default(),
            target_ratio: 5000,
            min_ratio: 1000,
            max_ratio: 9000,
        };

        let oracle = OracleParams {
            oracle_account: Pubkey::default(),
            oracle_type: OracleType::Test,
            max_price_error: 100,
            max_price_age_sec: 1,
        };

        let pricing = PricingParams {
            use_ema: true,
            trade_spread_long: 100,
            trade_spread_short: 100,
            swap_spread: 300,
            min_initial_leverage: 10000,
            max_leverage: 100000,
            max_payoff_mult: 10000,
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
            max_increase: 20000,
            max_decrease: 10000,
            swap: 100,
            add_liquidity: 200,
            remove_liquidity: 300,
            open_position: 100,
            close_position: 100,
            liquidation: 50,
            protocol_share: 25,
        };

        let custody = Custody {
            token_account: Pubkey::default(),
            mint: Pubkey::default(),
            decimals: 5,
            oracle,
            pricing,
            permissions,
            fees,
            ..Custody::default()
        };

        let position = Position {
            side: Side::Long,
            price: scale(120, Perpetuals::PRICE_DECIMALS),
            size_usd: scale(1000, Perpetuals::USD_DECIMALS),
            collateral_usd: scale(200, Perpetuals::USD_DECIMALS),
            locked_amount: scale(9, 5),
            collateral_amount: scale(1, 5),
            ..Position::default()
        };

        let token_price = OraclePrice {
            price: 123000,
            exponent: -3,
        };
        let token_ema_price = OraclePrice {
            price: 122000,
            exponent: -3,
        };

        (
            Pool {
                name: "Test Pool".to_string(),
                tokens: vec![token, token],
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

        assert_eq!(
            scale(1, Perpetuals::BPS_DECIMALS),
            pool.get_new_ratio(2000000, 0, &custody, &token_price)
                .unwrap()
        );

        assert_eq!(
            0,
            pool.get_new_ratio(0, 2000000, &custody, &token_price)
                .unwrap()
        );

        assert!(pool
            .get_new_ratio(2000000, 2000000, &custody, &token_price)
            .is_err());

        assert_eq!(0, pool.get_new_ratio(0, 0, &custody, &token_price).unwrap());

        pool.aum_usd = scale(5000000, Perpetuals::USD_DECIMALS) as u128;
        custody.assets.owned = scale(1000, custody.decimals);

        assert_eq!(
            703,
            pool.get_new_ratio(scale(2000, custody.decimals), 0, &custody, &token_price)
                .unwrap()
        );

        assert_eq!(
            124,
            pool.get_new_ratio(0, scale(500, custody.decimals), &custody, &token_price)
                .unwrap()
        );

        assert_eq!(
            0,
            pool.get_new_ratio(0, scale(1500, custody.decimals), &custody, &token_price)
                .unwrap()
        );

        assert_eq!(
            246,
            pool.get_new_ratio(0, 0, &custody, &token_price).unwrap()
        );
    }

    #[test]
    fn test_get_price() {
        let (pool, custody, _position, token_price, token_ema_price) = get_fixture();

        assert_eq!(
            OraclePrice {
                price: 124230,
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
                price: 120780,
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
    fn test_get_fee() {
        let (mut pool, mut custody, _position, token_price, _token_ema_price) = get_fixture();

        custody.fees.mode = FeesMode::Fixed;
        assert_eq!(
            scale_f64(0.2, custody.decimals),
            pool.get_fee(
                0,
                custody.fees.swap,
                scale(20, custody.decimals),
                0,
                &custody,
                &token_price
            )
            .unwrap()
        );

        custody.fees.mode = FeesMode::Linear;
        assert_eq!(
            scale_f64(0.6, custody.decimals),
            pool.get_fee(
                0,
                custody.fees.swap,
                scale(20, custody.decimals),
                0,
                &custody,
                &token_price
            )
            .unwrap()
        );

        pool.tokens[0].max_ratio = 10001;
        assert_eq!(
            scale_f64(0.6, custody.decimals),
            pool.get_fee(
                0,
                custody.fees.swap,
                scale(20, custody.decimals),
                0,
                &custody,
                &token_price
            )
            .unwrap()
        );

        pool.tokens[0].max_ratio = 9000;
        pool.aum_usd = scale(5000000, Perpetuals::USD_DECIMALS) as u128;
        custody.assets.owned = scale(10000, custody.decimals);
        assert_eq!(
            7200,
            pool.get_fee(
                0,
                custody.fees.swap,
                scale(20, custody.decimals),
                0,
                &custody,
                &token_price
            )
            .unwrap()
        );

        assert_eq!(
            scale_f64(196.0, custody.decimals),
            pool.get_fee(
                0,
                custody.fees.swap,
                scale(20000, custody.decimals),
                0,
                &custody,
                &token_price
            )
            .unwrap()
        );
    }

    #[test]
    fn test_get_pnl_usd() {
        let (pool, custody, mut position, token_price, token_ema_price) = get_fixture();

        assert_eq!(
            (scale_f64(3.9, Perpetuals::USD_DECIMALS), 0, 0),
            pool.get_pnl_usd(
                0,
                &position,
                &token_price,
                &token_ema_price,
                &custody,
                0,
                false
            )
            .unwrap()
        );

        position.price = scale(110, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            (scale_f64(53.9, Perpetuals::USD_DECIMALS), 0, 0),
            pool.get_pnl_usd(
                0,
                &position,
                &token_price,
                &token_ema_price,
                &custody,
                0,
                false
            )
            .unwrap()
        );

        position.price = scale(130, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            (0, scale_f64(46.1, Perpetuals::USD_DECIMALS), 0),
            pool.get_pnl_usd(
                0,
                &position,
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

        assert_eq!(
            scale_f64(4.9043, Perpetuals::BPS_DECIMALS),
            pool.get_leverage(0, &position, &token_price, &token_ema_price, &custody, 0)
                .unwrap()
        );

        position.price = scale(110, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale_f64(3.9385, Perpetuals::BPS_DECIMALS),
            pool.get_leverage(0, &position, &token_price, &token_ema_price, &custody, 0)
                .unwrap()
        );

        position.price = scale(130, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale_f64(6.4977, Perpetuals::BPS_DECIMALS),
            pool.get_leverage(0, &position, &token_price, &token_ema_price, &custody, 0)
                .unwrap()
        );

        position.price = scale(80, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale_f64(2.4758, Perpetuals::BPS_DECIMALS),
            pool.get_leverage(0, &position, &token_price, &token_ema_price, &custody, 0)
                .unwrap()
        );

        position.price = scale(0, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale_f64(1.2439, Perpetuals::BPS_DECIMALS),
            pool.get_leverage(0, &position, &token_price, &token_ema_price, &custody, 0)
                .unwrap()
        );

        position.price = scale(160, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            2564102,
            pool.get_leverage(0, &position, &token_price, &token_ema_price, &custody, 0)
                .unwrap()
        );

        position.price = scale(180, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            u64::MAX,
            pool.get_leverage(0, &position, &token_price, &token_ema_price, &custody, 0)
                .unwrap()
        );
    }

    #[test]
    fn test_get_liquidation_price() {
        let (pool, custody, mut position, token_price, _token_ema_price) = get_fixture();

        assert_eq!(
            scale_f64(101.0, Perpetuals::PRICE_DECIMALS),
            pool.get_liquidation_price(0, &position, &token_price, &custody)
                .unwrap()
        );

        position.price = scale(110, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale_f64(90.9, Perpetuals::PRICE_DECIMALS),
            pool.get_liquidation_price(0, &position, &token_price, &custody)
                .unwrap()
        );

        position.price = scale(130, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale_f64(111.1, Perpetuals::PRICE_DECIMALS),
            pool.get_liquidation_price(0, &position, &token_price, &custody)
                .unwrap()
        );

        position.price = scale(80, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale_f64(60.6, Perpetuals::PRICE_DECIMALS),
            pool.get_liquidation_price(0, &position, &token_price, &custody)
                .unwrap()
        );

        position.price = scale(0, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale_f64(0.0, Perpetuals::PRICE_DECIMALS),
            pool.get_liquidation_price(0, &position, &token_price, &custody)
                .unwrap()
        );

        position.price = scale(160, Perpetuals::PRICE_DECIMALS);
        assert_eq!(
            scale_f64(141.4, Perpetuals::PRICE_DECIMALS),
            pool.get_liquidation_price(0, &position, &token_price, &custody)
                .unwrap()
        );
    }

    #[test]
    fn test_get_close_amount() {
        let (pool, custody, position, token_price, token_ema_price) = get_fixture();

        assert_eq!(
            (
                scale_f64(0.82886, custody.decimals),
                0,
                scale_f64(3.9, Perpetuals::USD_DECIMALS),
                0
            ),
            pool.get_close_amount(
                0,
                &position,
                &token_price,
                &token_ema_price,
                &custody,
                position.size_usd / 2,
                0,
                false
            )
            .unwrap()
        );
    }

    #[test]
    fn test_get_interest_amount_usd() {
        let (pool, mut custody, mut position, _token_price, _token_ema_price) = get_fixture();

        custody.borrow_rate = BorrowRateParams {
            base_rate: 0,
            slope1: 80000,
            slope2: 120000,
            optimal_utilization: 800000000,
        };
        custody.assets.locked = scale(9, 5);
        custody.assets.owned = scale(10, 5);

        custody.update_borrow_rate(3600).unwrap();
        let interest = pool
            .get_interest_amount_usd(&position, &custody, 3600)
            .unwrap();
        assert_eq!(interest, 0);

        let interest = pool
            .get_interest_amount_usd(&position, &custody, 7200)
            .unwrap();
        assert_eq!(interest, scale_f64(0.14, Perpetuals::USD_DECIMALS));

        custody.update_borrow_rate(7200).unwrap();
        let interest = pool
            .get_interest_amount_usd(&position, &custody, 7199)
            .unwrap();
        assert_eq!(interest, scale_f64(0.14, Perpetuals::USD_DECIMALS));

        position.cumulative_interest_snapshot = 70000;
        let interest = pool
            .get_interest_amount_usd(&position, &custody, 7200)
            .unwrap();
        assert_eq!(interest, scale_f64(0.07, Perpetuals::USD_DECIMALS));
    }
}
