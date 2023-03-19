//! ClaimStake instruction handler

use crate::math;

use {
    crate::state::{cortex::Cortex, perpetuals::Perpetuals, stake::Stake},
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
    num::Zero,
    solana_program::log::sol_log_compute_units,
};

#[derive(Accounts)]
pub struct ClaimStake<'info> {
    #[account(mut)]
    pub caller: Signer<'info>,

    /// CHECK: verify through the `stake` account seed derivation
    #[account(mut)]
    pub owner: AccountInfo<'info>,

    // reward token account for the bounty receiver, if eligible
    #[account(
        mut,
        token::mint = stake_reward_token_mint,
        constraint = caller_reward_token_account.owner == caller.key()
    )]
    pub caller_reward_token_account: Box<Account<'info, TokenAccount>>,

    // reward token account of the stake owner
    #[account(
        mut,
        token::mint = stake_reward_token_mint,
        has_one = owner
    )]
    pub owner_reward_token_account: Box<Account<'info, TokenAccount>>,

    // staked token vault
    #[account(
        mut,
        token::mint = lm_token_mint,
        seeds = [b"stake_token_account"],
        bump = cortex.stake_token_account_bump
    )]
    pub stake_token_account: Box<Account<'info, TokenAccount>>,

    // staking reward token vault
    #[account(
        mut,
        token::mint = stake_reward_token_mint,
        seeds = [b"stake_reward_token_account"],
        bump = cortex.stake_reward_token_account_bump
    )]
    pub stake_reward_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"stake",
                 owner.key().as_ref()],
        bump = stake.bump
    )]
    pub stake: Box<Account<'info, Stake>>,

    #[account(
        mut,
        seeds = [b"cortex"],
        bump = cortex.bump,
        has_one = stake_reward_token_mint
    )]
    pub cortex: Box<Account<'info, Cortex>>,

    #[account(
        seeds = [b"perpetuals"],
        bump = perpetuals.perpetuals_bump
    )]
    pub perpetuals: Box<Account<'info, Perpetuals>>,

    #[account(
        mut,
        seeds = [b"lm_token_mint"],
        bump = cortex.lm_token_bump
    )]
    pub lm_token_mint: Box<Account<'info, Mint>>,

    #[account()]
    pub stake_reward_token_mint: Box<Account<'info, Mint>>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

pub fn claim_stake(ctx: Context<ClaimStake>) -> Result<bool> {
    let stake = ctx.accounts.stake.as_mut();
    let cortex = &mut ctx.accounts.cortex.as_mut();
    let did_claim: bool;

    let mut rate_sum = 0;
    msg!("Process resolved_staking_rounds");
    {
        // prints compute budget before
        sol_log_compute_units();
        let resolved_staking_rounds_len_before = cortex.resolved_staking_rounds.len();
        cortex.resolved_staking_rounds.retain_mut(|round| {
            // can the user claim the round rewards
            if stake.qualifies_for_rewards_from(round) {
                // Add to the rate
                rate_sum = math::checked_add(rate_sum, round.rate).unwrap();
                round.total_claim = math::checked_add(round.total_claim, stake.amount).unwrap();
            }
            // retain element if there is stake that has not been claimed yet by other participants
            let round_fully_claimed = round.total_claim == round.total_stake;
            !round_fully_claimed
        });
        let unretained_resolved_staking_rounds_amount = math::checked_sub(
            resolved_staking_rounds_len_before,
            cortex.resolved_staking_rounds.len(),
        )?;
        // prints compute budget after
        sol_log_compute_units();

        // realloc Cortex after update to its `stake_rounds` if needed
        if !unretained_resolved_staking_rounds_amount.is_zero() {
            msg!("Realloc Cortex");
            Perpetuals::realloc(
                ctx.accounts.caller.to_account_info(),
                ctx.accounts.caller.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                cortex.new_size(unretained_resolved_staking_rounds_amount),
                true,
            )?;
        }
    }

    {
        let rewards_amount = math::checked_mul(stake.amount, rate_sum)?;
        if !rewards_amount.is_zero() {
            msg!("Transfer reward tokens");
            let perpetuals = ctx.accounts.perpetuals.as_mut();

            // TODO - add a deadline as to when the  caller_reward_token_account is also rewarded with 1% of the stake

            perpetuals.transfer_tokens(
                ctx.accounts.stake_reward_token_account.to_account_info(),
                ctx.accounts.owner_reward_token_account.to_account_info(),
                ctx.accounts.transfer_authority.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                rewards_amount,
            )?;

            // refresh stake time
            stake.stake_time = ctx.accounts.perpetuals.get_time()?;

            // remove stake from current staking round
            cortex.current_staking_round.total_stake =
                math::checked_sub(cortex.current_staking_round.total_stake, stake.amount)?;

            // add stake to next staking round
            cortex.next_staking_round.total_stake =
                math::checked_add(cortex.next_staking_round.total_stake, stake.amount)?;

            did_claim = true;
        } else {
            msg!("No reward tokens to claim at this time");
            did_claim = false;
        }
    }
    msg!(
        "Cortex.resolved_staking_rounds after claim stake {:?}",
        cortex.resolved_staking_rounds
    );
    msg!(
        "Cortex.current_staking_round after claim stake {:?}",
        cortex.current_staking_round
    );
    msg!(
        "Cortex.next_staking_round after claim stake {:?}",
        cortex.next_staking_round
    );
    msg!("STATE after claim stake {:?}", stake);
    Ok(did_claim)
}
