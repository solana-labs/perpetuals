use {
    super::{cortex::SECONDS_PER_HOURS, user_staking::UserStaking},
    crate::math,
    anchor_lang::prelude::*,
};

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Debug)]
pub enum StakingType {
    LM = 1,
    LP = 2,
}

impl Default for StakingType {
    fn default() -> Self {
        Self::LM
    }
}

#[account]
#[derive(Default, Debug)]
pub struct Staking {
    //
    // Type of staking
    //
    pub staking_type: StakingType,
    //
    // Bumps
    //
    pub bump: u8,
    pub staked_token_vault_bump: u8,
    pub reward_token_vault_bump: u8,
    pub lm_reward_token_vault_bump: u8,
    //
    // Tokens in locked stake
    //
    pub nb_locked_tokens: u64,
    //
    // Token to stake
    //
    pub staked_token_mint: Pubkey,
    pub staked_token_decimals: u8,
    //
    // Token received as reward
    //
    pub reward_token_mint: Pubkey,
    pub reward_token_decimals: u8,
    //
    // Resolved amounts
    //
    // amount of rewards allocated to resolved rounds, claimable (excluding current/next round)
    pub resolved_reward_token_amount: u64,
    // amount of staked token locked in resolved rounds, claimable (excluding current/next round)
    pub resolved_staked_token_amount: u64,
    // amount of lm rewards allocated to resolved rounds, claimable (excluding current/next round)
    pub resolved_lm_reward_token_amount: u64,
    // amount of lm staked token locked in resolved rounds, claimable (excluding current/next round)
    pub resolved_lm_staked_token_amount: u64,
    //
    // Staking rounds
    //
    pub current_staking_round: StakingRound,
    pub next_staking_round: StakingRound,
    // must be the last element of the struct for reallocs
    pub resolved_staking_rounds: Vec<StakingRound>,
}

#[derive(Default, Debug, Clone, AnchorSerialize, AnchorDeserialize, PartialEq)]
pub struct StakingRound {
    pub start_time: i64,
    //
    pub rate: u64, // the amount of reward you get per staked stake-token for that round - set at Round's resolution
    pub total_stake: u64, // - set at Round's resolution
    pub total_claim: u64, // - set at Round's resolution
    //
    pub lm_rate: u64, // the amount of lm reward you get per staked stake-token for that round - set at Round's resolution
    pub lm_total_stake: u64, // - set at Round's resolution
    pub lm_total_claim: u64, // - set at Round's resolution
}

impl StakingRound {
    pub const LEN: usize = std::mem::size_of::<StakingRound>();
    // a staking round can be resolved after at least 6 hours
    const ROUND_MIN_DURATION_HOURS: i64 = 6;
    pub const ROUND_MIN_DURATION_SECONDS: i64 = Self::ROUND_MIN_DURATION_HOURS * SECONDS_PER_HOURS;
    // A UserStaking account max age is 365, this is due to computing limit in the claim instruction.
    // This is also arbitrarily used as the max theoretical amount of staking rounds
    // stored if all were persisting (rounds get cleaned up once their rewards are fully claimed by their participants).
    // This is done to ensure the Cortex.resolved_staking_rounds doesn't grow out of proportion, primarily to facilitate
    // the fetching from front end.
    pub const MAX_RESOLVED_ROUNDS: usize = ((UserStaking::MAX_AGE_SECONDS / SECONDS_PER_HOURS)
        / Self::ROUND_MIN_DURATION_HOURS) as usize;

    pub fn new(start_time: i64) -> Self {
        Self {
            start_time,
            rate: u64::MIN,
            total_stake: u64::MIN,
            total_claim: u64::MIN,
            lm_rate: u64::MIN,
            lm_total_stake: u64::MIN,
            lm_total_claim: u64::MIN,
        }
    }
}

impl Staking {
    pub const LEN: usize = std::mem::size_of::<Staking>();

    pub fn current_staking_round_is_resolvable(&self, current_time: i64) -> Result<bool> {
        Ok(current_time
            >= math::checked_add(
                self.current_staking_round.start_time,
                StakingRound::ROUND_MIN_DURATION_SECONDS,
            )?)
    }

    pub fn size(&self) -> usize {
        Staking::LEN + self.resolved_staking_rounds.len() * StakingRound::LEN
    }
}
