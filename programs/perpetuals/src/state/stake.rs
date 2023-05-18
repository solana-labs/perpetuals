//! Stake state and routines
//!
//! Stake represent the LM staking account of a user of the platform.
//! Staking of LM token grant access to a share of the platform revenues
//! proportionnal to the amount of staked tokens.
//! To ensure fair distribution, rewards are per rounds.
//! A round has a fixed minimum duration, after which it will be available for resolution.
//! Resolution of a round closes it, define the amount of reward per staked token during that round,
//! and initialize the next staking round.
//!
//! User can claim their `Stake`, by doing so the program will read the vec of `StakeRound`s in the `Cortex`
//! and determined based on the `Stake.inception_timestamp` if the user is elegible for the round rewards.
//! The `StakeRound` will increase it's `token_claim` property, and once it matches the `token_stake` one,
//! will remove itself from the record.
//!
//! Since there is a hard limitation on the data stored onchain on solana (10mb per accounts), the `stake_rounds`
//! property of the `Cortex` have a upper limit. Once the limit is nearing, the `claim_stake` for `Stake`
//! where the `inception_timestamp` is old enough will offer % of the reward to the caller, similar to a liquidation.
//!
//! This should ensure that the `stake_rounds` vec never grow beyond what's storable, in a decentralized fashion.
//! (Adrena will run a claim-bot until decentralized enough, but anyone can partake)
//!

use {
    super::{
        cortex::{StakingRound, DAYS_PER_YEAR, HOURS_PER_DAY, SECONDS_PER_HOURS},
        perpetuals::Perpetuals,
    },
    crate::math,
    anchor_lang::prelude::*,
    std::cmp::max,
};

#[account]
#[derive(Default, Debug)]
pub struct Stake {
    pub amount: u64,
    pub bump: u8,
    // this value is refreshed during each call to add_stake, remove_stake or claim_stake
    pub stake_time: i64,
}

// define if the additionnal caller on claim_reward is eligible for bounty
pub enum BountyStage {
    NoReward,
    StageOne,
    StageTwo,
}

impl BountyStage {
    const fn reward_bps(self) -> u64 {
        match self {
            BountyStage::NoReward => 0,
            BountyStage::StageOne => 10,  // 0.1%
            BountyStage::StageTwo => 500, // 5%
        }
    }
}

impl Stake {
    pub const LEN: usize = 8 + std::mem::size_of::<Stake>();

    // The max age of a Stake account in the system, 365 days
    pub const MAX_AGE_SECONDS: i64 = DAYS_PER_YEAR * HOURS_PER_DAY * SECONDS_PER_HOURS;

    pub fn qualifies_for_rewards_from(&self, staking_round: &StakingRound) -> bool {
        msg!("self.stake_time: {}", self.stake_time);
        msg!("staking_round.start_time: {}", staking_round.start_time);
        self.stake_time > 0 && self.stake_time < staking_round.start_time
    }

    pub fn bounty_stage(&self, current_time: i64) -> Result<BountyStage> {
        let stake_duration =
            math::checked_as_u64(math::checked_sub(current_time, self.stake_time)?)?;
        let ratio = math::checked_decimal_div(
            math::checked_as_u64(stake_duration)?,
            0 as i32,
            math::checked_as_u64(Self::MAX_AGE_SECONDS)?,
            0 as i32,
            -(Perpetuals::BPS_DECIMALS as i32),
        )?;

        Ok(match ratio {
            0..=8999 => BountyStage::NoReward,    // 0-90%
            9000..=9499 => BountyStage::StageOne, // 90-95%
            _ => BountyStage::StageTwo,           // 95% +
        })
    }

    // This is done to ensure the Cortex.resolved_staking_rounds doesn't grow out of proportion, primarily to facilitate
    // the fetching from front end, and because solana on chain storage is size limited.
    // This function calculates the allocatted share of rewards the caller of the claim IX is entitled to,
    // incentivizing claiming for other participants with the use of this bounty.
    // The goal of this is to prevent users to loose their rewards, as the StakingRound will be dropped after that. This
    // can be seen as a sort of "auto restaking".
    // The bounty amount depend of the age of the Stake account, the closer to `MAX_AGE_SECONDS` the higher.
    // Bounty starts at 90%-95% for 0.1%, then gets bumped from 95%-100% to 5%
    pub fn get_claim_stake_caller_reward_token_amounts(
        &self,
        reward_token_amount: u64,
        current_time: i64,
    ) -> Result<u64> {
        let bounty_stage = self.bounty_stage(current_time)?;
        match bounty_stage {
            BountyStage::NoReward => Ok(0),
            BountyStage::StageOne => Ok(max(
                1,
                math::checked_decimal_mul(
                    reward_token_amount,
                    1,
                    bounty_stage.reward_bps(),
                    -(Perpetuals::BPS_DECIMALS as i32),
                    0,
                )?,
            )),
            BountyStage::StageTwo => Ok(max(
                1,
                math::checked_as_u64(math::checked_div(
                    math::checked_mul(
                        reward_token_amount as u128,
                        bounty_stage.reward_bps() as u128,
                    )?,
                    Perpetuals::BPS_POWER,
                )?)?,
            )),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn get_fixture_stake(stake_time: i64) -> Stake {
        Stake {
            amount: 0,
            bump: 255,
            stake_time,
        }
    }

    #[test]
    fn test_get_claim_stake_caller_reward_token_amounts() {
        let reward_token_amount = 100; // native units

        // out of the bounty period
        let time = 69420;
        let stake = get_fixture_stake(time);
        let current_time = time + 0;
        let bounty_amount = stake
            .get_claim_stake_caller_reward_token_amounts(reward_token_amount, current_time)
            .unwrap();
        assert_eq!(bounty_amount, 0);

        // in of the bounty period phase one
        let time = 69420;
        let stake = get_fixture_stake(time);
        let current_time = time + 28386000; //90% of a year
        let bounty_amount_phase_one = stake
            .get_claim_stake_caller_reward_token_amounts(reward_token_amount, current_time)
            .unwrap();
        assert_ne!(bounty_amount_phase_one, 0);

        // in of the bounty period phase two
        let time = 69420;
        let stake = get_fixture_stake(time);
        let current_time = time + 29979079; // 95% of a year
        let bounty_amount_phase_two = stake
            .get_claim_stake_caller_reward_token_amounts(reward_token_amount, current_time)
            .unwrap();
        assert!(bounty_amount_phase_one < bounty_amount_phase_two);
    }
}
