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
        cortex::{StakingRound, HOURS_PER_DAY, SECONDS_PER_HOURS},
        perpetuals::Perpetuals,
    },
    crate::{error::PerpetualsError, math},
    anchor_lang::prelude::*,
};

pub const STAKING_THREAD_AUTHORITY_SEED: &[u8] = b"staking-thread-authority";

#[account]
#[derive(Default, Debug)]
pub struct Staking {
    pub bump: u8,
    pub thread_authority_bump: u8,

    pub liquid_stake: LiquidStake,
    pub locked_stakes: Vec<LockedStake>,
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct LiquidStake {
    pub amount: u64,
    pub stake_time: i64,

    // Time used for claim purpose, to know wherever the stake is elligible for round reward
    pub claim_time: i64,
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Debug)]
pub struct LockedStake {
    pub amount: u64,
    pub stake_time: i64,

    // Last time tokens have been claimed for this stake
    pub claim_time: i64,

    // In seconds
    pub lock_duration: u64,

    // In BPS
    pub base_reward_multiplier: u32,
    pub lm_token_reward_multiplier: u32,
    pub vote_multiplier: u32,

    // Persisted data to save-up computation during claim etc.
    // amount with real yield multiplier applied to it
    pub amount_with_multiplier: u64,

    // locked stake needs to be resolved before removing it
    // doesn't apply to liquid stake (lock_duration == 0)
    pub resolved: bool,
}

impl LiquidStake {
    pub const LEN: usize = std::mem::size_of::<LockedStake>();

    pub fn qualifies_for_rewards_from(&self, staking_round: &StakingRound) -> bool {
        msg!("self.stake_time: {}", self.stake_time);
        msg!("staking_round.start_time: {}", staking_round.start_time);

        self.stake_time > 0
            && self.stake_time < staking_round.start_time
            && (self.claim_time == 0 || self.claim_time < staking_round.start_time)
    }
}

impl LockedStake {
    pub const LEN: usize = std::mem::size_of::<LockedStake>();

    pub fn qualifies_for_rewards_from(&self, staking_round: &StakingRound) -> bool {
        self.stake_time > 0
            && self.stake_time < staking_round.start_time
            && (self.claim_time == 0 || self.claim_time < staking_round.start_time)
    }

    pub fn has_ended(&self, current_time: i64) -> bool {
        (self.stake_time + self.lock_duration as i64) < current_time
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct LockedStakingOption {
    pub locked_days: u32,
    pub base_reward_multiplier: u32,
    pub lm_token_reward_multiplier: u32,
    pub vote_multiplier: u32,
}

impl LockedStakingOption {
    pub fn calculate_end_of_staking(&self, start: i64) -> Result<i64> {
        math::checked_add(
            start,
            math::checked_mul(SECONDS_PER_HOURS * HOURS_PER_DAY, self.locked_days as i64)?,
        )
    }
}

// List of valid locked staking options and the related multipliers
pub const LOCKED_STAKING_OPTIONS: [&'static LockedStakingOption; 6] = [
    &LockedStakingOption {
        locked_days: 30,
        base_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.25) as u32,
        lm_token_reward_multiplier: Perpetuals::BPS_POWER as u32,
        vote_multiplier: (Perpetuals::BPS_POWER as f64 * 1.21) as u32,
    },
    &LockedStakingOption {
        locked_days: 60,
        base_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.56) as u32,
        lm_token_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.25) as u32,
        vote_multiplier: (Perpetuals::BPS_POWER as f64 * 1.33) as u32,
    },
    &LockedStakingOption {
        locked_days: 90,
        base_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.95) as u32,
        lm_token_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.56) as u32,
        vote_multiplier: (Perpetuals::BPS_POWER as f64 * 1.46) as u32,
    },
    &LockedStakingOption {
        locked_days: 180,
        base_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 2.44) as u32,
        lm_token_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.95) as u32,
        vote_multiplier: (Perpetuals::BPS_POWER as f64 * 1.61) as u32,
    },
    &LockedStakingOption {
        locked_days: 360,
        base_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 3.05) as u32,
        lm_token_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 2.44) as u32,
        vote_multiplier: (Perpetuals::BPS_POWER as f64 * 1.78) as u32,
    },
    &LockedStakingOption {
        locked_days: 720,
        base_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 3.81) as u32,
        lm_token_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 3.05) as u32,
        vote_multiplier: (Perpetuals::BPS_POWER as f64 * 1.95) as u32,
    },
];

impl Staking {
    pub const LEN: usize = 8 + std::mem::size_of::<Staking>();

    // The max age of a Staking account in the system, 20 days
    pub const MAX_AGE_SECONDS: i64 = 20 * HOURS_PER_DAY * SECONDS_PER_HOURS;

    pub fn get_locked_staking_option(&self, locked_days: u32) -> Result<LockedStakingOption> {
        let staking_option = LOCKED_STAKING_OPTIONS
            .into_iter()
            .find(|period| period.locked_days == locked_days);

        require!(
            staking_option.is_some(),
            PerpetualsError::InvalidStakingLockingTime
        );

        Ok(*staking_option.unwrap())
    }

    // returns the current size of the Staking
    pub fn size(&self) -> usize {
        return Staking::LEN + self.locked_stakes.len() * LockedStake::LEN;
    }

    // returns the new size of the structure after adding/removing stakings
    pub fn new_size(&self, staking_delta: i32) -> Result<usize> {
        math::checked_as_usize(math::checked_add(
            self.size() as i32,
            math::checked_mul(staking_delta, LockedStake::LEN as i32)?,
        )?)
    }
}
