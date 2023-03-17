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

use anchor_lang::prelude::*;

#[account]
#[derive(Default, Debug)]
pub struct Stake {
    pub amount: u64,
    pub bump: u8,
    // this value is refreshed during each call to add_stake, remove_stake or claim_stake
    pub stake_time: i64,
}

/// Stake
impl Stake {
    pub const LEN: usize = 8 + std::mem::size_of::<Stake>();
}
