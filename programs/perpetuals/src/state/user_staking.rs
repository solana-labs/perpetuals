use {
    super::{
        cortex::{HOURS_PER_DAY, SECONDS_PER_HOURS},
        perpetuals::Perpetuals,
        staking::{StakingRound, StakingType},
    },
    crate::{error::PerpetualsError, math},
    anchor_lang::prelude::*,
};

pub const USER_STAKING_THREAD_AUTHORITY_SEED: &[u8] = b"user-staking-thread-authority";

pub const CLOCKWORK_PAYER_PUBKEY: &str = "C1ockworkPayer11111111111111111111111111111";

#[account]
#[derive(Default, Debug)]
pub struct UserStaking {
    pub bump: u8,
    pub thread_authority_bump: u8,

    pub stakes_claim_cron_thread_id: u64,

    pub liquid_stake: LiquidStake,
    pub locked_stakes: Vec<LockedStake>,
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct LiquidStake {
    pub amount: u64,
    pub stake_time: i64,

    // Time used for claim purpose, to know wherever the stake is elligible for round reward
    pub claim_time: i64,

    // When user add stake when a stake is already live
    pub overlap_time: i64,
    pub overlap_amount: u64,
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
    pub reward_multiplier: u32,
    pub lm_reward_multiplier: u32,
    pub vote_multiplier: u32,

    // Persisted data to save-up computation during claim etc.
    // amount with base reward multiplier applied to it
    pub amount_with_reward_multiplier: u64,
    // amount with base reward multiplier applied to it
    pub amount_with_lm_reward_multiplier: u64,

    // locked stake needs to be resolved before removing it
    // doesn't apply to liquid stake (lock_duration == 0)
    pub resolved: bool,

    pub stake_resolution_thread_id: u64,
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
    pub reward_multiplier: u32,
    pub lm_reward_multiplier: u32,
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
pub const LOCKED_LM_STAKING_OPTIONS: [&LockedStakingOption; 6] = [
    &LockedStakingOption {
        locked_days: 30,
        reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.25) as u32,
        lm_reward_multiplier: Perpetuals::BPS_POWER as u32,
        vote_multiplier: (Perpetuals::BPS_POWER as f64 * 1.21) as u32,
    },
    &LockedStakingOption {
        locked_days: 60,
        reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.56) as u32,
        lm_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.25) as u32,
        vote_multiplier: (Perpetuals::BPS_POWER as f64 * 1.33) as u32,
    },
    &LockedStakingOption {
        locked_days: 90,
        reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.95) as u32,
        lm_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.56) as u32,
        vote_multiplier: (Perpetuals::BPS_POWER as f64 * 1.46) as u32,
    },
    &LockedStakingOption {
        locked_days: 180,
        reward_multiplier: (Perpetuals::BPS_POWER as f64 * 2.44) as u32,
        lm_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.95) as u32,
        vote_multiplier: (Perpetuals::BPS_POWER as f64 * 1.61) as u32,
    },
    &LockedStakingOption {
        locked_days: 360,
        reward_multiplier: (Perpetuals::BPS_POWER as f64 * 3.05) as u32,
        lm_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 2.44) as u32,
        vote_multiplier: (Perpetuals::BPS_POWER as f64 * 1.78) as u32,
    },
    &LockedStakingOption {
        locked_days: 720,
        reward_multiplier: (Perpetuals::BPS_POWER as f64 * 3.81) as u32,
        lm_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 3.05) as u32,
        vote_multiplier: (Perpetuals::BPS_POWER as f64 * 1.95) as u32,
    },
];

pub const LOCKED_LP_STAKING_OPTIONS: [&LockedStakingOption; 6] = [
    &LockedStakingOption {
        locked_days: 30,
        reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.3) as u32,
        lm_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.3) as u32,
        vote_multiplier: 0,
    },
    &LockedStakingOption {
        locked_days: 60,
        reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.7) as u32,
        lm_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.7) as u32,
        vote_multiplier: 0,
    },
    &LockedStakingOption {
        locked_days: 90,
        reward_multiplier: (Perpetuals::BPS_POWER as f64 * 2.2) as u32,
        lm_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 2.2) as u32,
        vote_multiplier: 0,
    },
    &LockedStakingOption {
        locked_days: 180,
        reward_multiplier: (Perpetuals::BPS_POWER as f64 * 2.9) as u32,
        lm_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 2.9) as u32,
        vote_multiplier: 0,
    },
    &LockedStakingOption {
        locked_days: 360,
        reward_multiplier: (Perpetuals::BPS_POWER as f64 * 3.7) as u32,
        lm_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 3.7) as u32,
        vote_multiplier: 0,
    },
    &LockedStakingOption {
        locked_days: 720,
        reward_multiplier: (Perpetuals::BPS_POWER as f64 * 4.8) as u32,
        lm_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 4.8) as u32,
        vote_multiplier: 0,
    },
];

impl UserStaking {
    pub const LEN: usize = 8 + std::mem::size_of::<UserStaking>();

    // The max age of a UserStaking account in the system, 9 days
    pub const MAX_AGE_SECONDS: i64 = 8 * HOURS_PER_DAY * SECONDS_PER_HOURS;

    // Run cron every 7 days, leaving 1 days of buffering in case cron doesn't execute as it should have
    pub const AUTO_CLAIM_CRON_DAYS_PERIODICITY: u8 = 7;

    // Cover ~10 years of auto-claim fees (530 calls * 7 days between calls = 3710 days covered ~= 10.1643835616 years)
    pub const AUTO_CLAIM_FEE_COVERED_CALLS: u64 = 530;

    // Fee paid for the execution of one automated action using clockwork
    pub const AUTOMATION_EXEC_FEE: u64 = 1_000;

    pub fn get_locked_staking_option(
        &self,
        locked_days: u32,
        staking_type: StakingType,
    ) -> Result<LockedStakingOption> {
        let options = if staking_type == StakingType::LM {
            LOCKED_LM_STAKING_OPTIONS
        } else {
            LOCKED_LP_STAKING_OPTIONS
        };

        let staking_option = options
            .into_iter()
            .find(|period| period.locked_days == locked_days);

        require!(
            staking_option.is_some(),
            PerpetualsError::InvalidStakingLockingTime
        );

        Ok(*staking_option.unwrap())
    }

    // returns the current size of the UserStaking
    pub fn size(&self) -> usize {
        UserStaking::LEN + self.locked_stakes.len() * LockedStake::LEN
    }

    // returns the new size of the structure after adding/removing stakings
    pub fn new_size(&self, staking_delta: i32) -> Result<usize> {
        math::checked_as_usize(math::checked_add(
            self.size() as i32,
            math::checked_mul(staking_delta, LockedStake::LEN as i32)?,
        )?)
    }
}
