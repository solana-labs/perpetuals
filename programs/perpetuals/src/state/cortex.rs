//! Cortex state and routines

use {
    super::{perpetuals::Perpetuals, staking::Staking, vest::Vest},
    crate::math,
    anchor_lang::prelude::*,
    anchor_spl::token::Mint,
};

pub const HOURS_PER_DAY: i64 = 24;
pub const SECONDS_PER_HOURS: i64 = 3600;

#[account]
#[derive(Default, Debug)]
pub struct Cortex {
    //
    // Bumps
    //
    pub bump: u8,
    pub lm_token_bump: u8,
    pub governance_token_bump: u8,
    //
    // Time
    //
    pub inception_epoch: u64,
    //
    // Governance
    //
    pub governance_program: Pubkey,
    pub governance_realm: Pubkey,
    //
    // Vesting
    //
    pub vests: Vec<Pubkey>,
    //
    // Lm tokens minting rules
    //
    pub core_contributor_bucket_allocation: u64,
    pub core_contributor_bucket_minted_amount: u64,
    pub dao_treasury_bucket_allocation: u64,
    pub dao_treasury_bucket_minted_amount: u64,
    pub pol_bucket_allocation: u64,
    pub pol_bucket_minted_amount: u64,
    pub ecosystem_bucket_allocation: u64,
    pub ecosystem_bucket_minted_amount: u64,
}

// Represent the fee distribution between:
//
// - ADX stakers
// - ALP holders
// - ALP locked stakers
#[derive(Default, Debug, Clone, Copy)]
pub struct FeeDistribution {
    pub lm_stakers_fee: u64,
    pub locked_lp_stakers_fee: u64,
    pub lp_organic_fee: u64,
}

impl Cortex {
    pub const LEN: usize = 8 + std::mem::size_of::<Cortex>();
    const INCEPTION_EMISSION_RATE: u64 = Perpetuals::RATE_POWER as u64; // 100%
    pub const FEE_TO_REWARD_RATIO_BPS: u8 = 10; //  0.10% of fees paid become rewards
    pub const LM_DECIMALS: u8 = Perpetuals::USD_DECIMALS;
    pub const GOVERNANCE_DECIMALS: u8 = Perpetuals::USD_DECIMALS;
    // a limit is needed to keep the Cortex size deterministic
    pub const MAX_ONGOING_VESTS: usize = 64;
    // length of our epoch relative to Solana epochs (1 Solana epoch is ~2-3 days)
    const ADRENA_EPOCH: u8 = 10;

    // Fee distributions, in BPS
    pub const LM_STAKERS_FEE_SHARE_AMOUNT: u128 = 3_000;
    pub const LP_HOLDERS_ORGANIC_FEE_SHARE_AMOUNT: u128 = 7_000;

    pub fn calculate_fee_distribution(
        &self,
        fee_amount: u64,
        lp_token_mint: &Account<Mint>,
        lp_staking: &Account<Staking>,
    ) -> Result<FeeDistribution> {
        let lm_stakers_fee = self.get_lm_stakers_fee(fee_amount)?;
        let lp_organic_fee = self.get_lp_organic_fee(fee_amount)?;

        let locked_lp_stakers_fee =
            self.get_locked_lp_stakers_fee(lp_organic_fee, lp_token_mint, lp_staking)?;

        Ok(FeeDistribution {
            lm_stakers_fee,
            locked_lp_stakers_fee,
            lp_organic_fee,
        })
    }

    fn get_lm_stakers_fee(&self, fee_amount: u64) -> Result<u64> {
        math::checked_as_u64(math::checked_div(
            math::checked_mul(fee_amount as u128, Cortex::LM_STAKERS_FEE_SHARE_AMOUNT)?,
            Perpetuals::BPS_POWER,
        )?)
    }

    fn get_lp_organic_fee(&self, fee_amount: u64) -> Result<u64> {
        math::checked_as_u64(math::checked_div(
            math::checked_mul(
                fee_amount as u128,
                Cortex::LP_HOLDERS_ORGANIC_FEE_SHARE_AMOUNT,
            )?,
            Perpetuals::BPS_POWER,
        )?)
    }

    fn get_locked_lp_stakers_fee(
        &self,
        lp_organic_fee: u64,
        lp_token_mint: &Account<Mint>,
        lp_staking: &Account<Staking>,
    ) -> Result<u64> {
        if lp_organic_fee == 0 {
            return Ok(0);
        }

        let non_locked_staked_tokens =
            math::checked_sub(lp_token_mint.supply, lp_staking.nb_locked_tokens)?;

        let total_lp_holders_shares = math::checked_add(
            non_locked_staked_tokens,
            lp_staking.current_staking_round.total_stake,
        )?;

        if total_lp_holders_shares == 0 {
            return Ok(0);
        }

        let share_per_token = math::checked_div(
            math::checked_mul(lp_organic_fee as u128, Perpetuals::RATE_POWER)?,
            total_lp_holders_shares as u128,
        )?;

        math::checked_as_u64(math::checked_div(
            math::checked_mul(
                share_per_token,
                lp_staking.current_staking_round.total_stake as u128,
            )?,
            Perpetuals::RATE_POWER,
        )?)
    }

    pub fn get_swap_lm_rewards_amounts(&self, (fee_in, fee_out): (u64, u64)) -> Result<(u64, u64)> {
        Ok((
            self.get_lm_rewards_amount(fee_in)?,
            self.get_lm_rewards_amount(fee_out)?,
        ))
    }

    // lm rewards amount is a portion of fees paid, scaled down by the current epoch emission rate
    pub fn get_lm_rewards_amount(&self, fee_amount: u64) -> Result<u64> {
        let base_reward_amount = math::checked_as_u64(math::checked_div(
            math::checked_mul(fee_amount as u128, Self::FEE_TO_REWARD_RATIO_BPS as u128)?,
            Perpetuals::BPS_POWER,
        )?)?;
        let emission_rate = Self::get_emission_rate(self.inception_epoch, self.get_epoch()?)?;
        let epoch_adjusted_reward_amount = math::checked_as_u64(math::checked_div(
            math::checked_mul(base_reward_amount as u128, emission_rate as u128)?,
            Perpetuals::RATE_POWER,
        )?)?;
        Ok(epoch_adjusted_reward_amount)
    }

    fn get_emission_rate(inception_epoch: u64, current_epoch: u64) -> Result<u64> {
        let elapsed_epochs = std::cmp::max(math::checked_sub(current_epoch, inception_epoch)?, 1);

        math::checked_div(
            Self::INCEPTION_EMISSION_RATE,
            std::cmp::max(elapsed_epochs / Cortex::ADRENA_EPOCH as u64, 1),
        )
    }

    pub fn get_epoch(&self) -> Result<u64> {
        let epoch = solana_program::sysvar::clock::Clock::get()?.epoch;
        Ok(epoch)
    }

    // returns the current size of the Cortex
    pub fn size(&self) -> usize {
        Cortex::LEN + self.vests.len() * Vest::LEN
    }
}

#[cfg(test)]
mod test {
    use {super::*, proptest::prelude::*};

    #[test]
    fn test_get_emission_rate() {
        proptest!(|(inception_epoch: u32, epoches_elapsed: u32)| {
            let current_epoch = inception_epoch as u64 + epoches_elapsed as u64;
            let divider = match current_epoch {
                0 => 1,
                _ => epoches_elapsed as u64 / 10
            };
            assert_eq!(
                Cortex::get_emission_rate(inception_epoch as u64, current_epoch).unwrap(),
                Cortex::INCEPTION_EMISSION_RATE / divider
            );
        });
    }
}
