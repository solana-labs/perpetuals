//! Cortex state and routines

use anchor_lang::prelude::*;

use crate::math;

use super::perpetuals::Perpetuals;

#[account]
#[derive(Default, Debug)]
pub struct Cortex {
    // emission have implied RATE_DECIMALS decimals
    pub cortex_bump: u8,
    pub lm_token_bump: u8,
    pub inception_epoch: u64,
}

/// Cortex
impl Cortex {
    pub const LEN: usize = 8 + std::mem::size_of::<Cortex>();
    const INCEPTION_EMISSION_RATE: u64 = Perpetuals::RATE_POWER as u64; // 100%
    pub const FEE_TO_REWARD_RATIO_BPS: u8 = 10; //  0.10% of fees paid become rewards

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
        let elapsed_epoches = std::cmp::max(math::checked_sub(current_epoch, inception_epoch)?, 1);

        math::checked_div(
            Self::INCEPTION_EMISSION_RATE as u64,
            std::cmp::max(elapsed_epoches / 10, 1) as u64,
        )
    }

    #[cfg(feature = "test")]
    pub fn get_epoch(&self) -> Result<u64> {
        Ok(20)
    }

    #[cfg(not(feature = "test"))]
    pub fn get_epoch(&self) -> Result<u64> {
        let epoch = solana_program::sysvar::clock::Clock::get()?.epoch;
        if epoch > 0 {
            Ok(epoch)
        } else {
            Err(ProgramError::InvalidAccountData.into())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use proptest::prelude::*;

    fn get_fixture() -> Cortex {
        let cortex = Cortex {
            cortex_bump: 255,
            lm_token_bump: 255,
            inception_epoch: 0,
        };
        cortex
    }

    fn scale_f64(amount: f64, decimals: u8) -> u64 {
        math::checked_as_u64(
            math::checked_float_mul(amount, 10u64.pow(decimals as u32) as f64).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn test_get_lm_rewards_amount() {
        let cortex = get_fixture();

        assert_eq!(
            cortex
                .get_lm_rewards_amount(scale_f64(2.5, Perpetuals::USD_DECIMALS))
                .unwrap(),
            scale_f64(0.00125, Perpetuals::USD_DECIMALS)
        );

        assert_eq!(cortex.get_lm_rewards_amount(0).unwrap(), 0);
    }

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
