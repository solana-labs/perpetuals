//! Cortex state and routines

use {super::perpetuals::Perpetuals, crate::math, anchor_lang::prelude::*};

// lenght of our epoch relative to Solana epochs (1 Solana epoch is ~2-3 days)
const ADRENA_EPOCH: u8 = 10;
pub const STAKING_ROUND_MIN_DURATION: i64 = 3600 * 6;

#[account]
#[derive(Default, Debug)]
pub struct Cortex {
    pub vests: Vec<Pubkey>,
    pub bump: u8,
    pub lm_token_bump: u8,
    pub stake_token_account_bump: u8,
    pub inception_epoch: u64,
    pub governance_program: Pubkey,
    pub governance_realm: Pubkey,
    pub staking_rounds: Vec<StakingRound>,
}

#[derive(Default, Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct StakingRound {
    pub timestamp_start: i64,
    pub rate: u64, // the amount of reward you get per staked stake-token for that round - set at Round's resolution
    pub total_stake: u64, // - set at Round's resolution
    pub total_claim: u64, // - set at Round's resolution
}

impl StakingRound {
    pub fn new(current_time: i64) -> Self {
        Self {
            timestamp_start: current_time,
            rate: u64::MIN,
            total_stake: u64::MIN,
            total_claim: u64::MIN,
        }
    }
}

/// Cortex
impl Cortex {
    pub const LEN: usize = 8 + std::mem::size_of::<Cortex>();
    const INCEPTION_EMISSION_RATE: u64 = Perpetuals::RATE_POWER as u64; // 100%
    pub const FEE_TO_REWARD_RATIO_BPS: u8 = 10; //  0.10% of fees paid become rewards
    pub const LM_DECIMALS: u8 = Perpetuals::USD_DECIMALS;
    pub const STAKE_REDEEMABLE_DECIMALS: u8 = Perpetuals::USD_DECIMALS; // LM token staking redeemable

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
            std::cmp::max(elapsed_epochs / ADRENA_EPOCH as u64, 1),
        )
    }

    pub fn get_epoch(&self) -> Result<u64> {
        let epoch = solana_program::sysvar::clock::Clock::get()?.epoch;
        Ok(epoch)
    }

    pub fn get_latest_staking_round_mut(&mut self) -> Result<&mut StakingRound> {
        match self.staking_rounds.last_mut() {
            Some(current_staking_round) => Ok(current_staking_round),
            None => Err(ProgramError::InvalidAccountData.into()),
        }
    }
}

#[cfg(test)]
mod test {
    use {super::*, proptest::prelude::*};

    // fn get_fixture() -> Cortex {
    //     Cortex {
    //         vests: Vec::new(),
    //         bump: 255,
    //         lm_token_bump: 255,
    //         inception_epoch: 0,
    //     }
    // }

    // fn scale_f64(amount: f64, decimals: u8) -> u64 {
    //     math::checked_as_u64(
    //         math::checked_float_mul(amount, 10u64.pow(decimals as u32) as f64).unwrap(),
    //     )
    //     .unwrap()
    // }

    // Need to move epochs, thiw would be epoch 10
    // #[test]
    // fn test_get_lm_rewards_amount() {
    //     let cortex = get_fixture();

    //     assert_eq!(
    //         cortex
    //             .get_lm_rewards_amount(scale_f64(2.5, Perpetuals::USD_DECIMALS))
    //             .unwrap(),
    //         scale_f64(0.00125, Perpetuals::USD_DECIMALS)
    //     );

    //     assert_eq!(cortex.get_lm_rewards_amount(0).unwrap(), 0);
    // }

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
