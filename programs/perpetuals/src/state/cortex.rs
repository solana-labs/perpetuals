//! Cortex state and routines

use {
    super::{perpetuals::Perpetuals, vest::Vest},
    crate::math,
    anchor_lang::prelude::*,
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
