//! ClaimStake instruction handler

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

    #[account(mut)]
    pub owner: AccountInfo<'info>,

    // reward token account for the bounty receiver, if eligible
    #[account(
        mut,
        token::mint = lm_token_mint,
        constraint = caller_reward_token_account.owner == caller.key()
    )]
    pub caller_reward_token_account: Box<Account<'info, TokenAccount>>,

    // reward token account of the stake owner
    #[account(
        mut,
        token::mint = lm_token_mint,
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

pub fn claim_stake(ctx: Context<ClaimStake>) -> Result<()> {
    let stake = ctx.accounts.stake.as_mut();
    let cortex = &mut ctx.accounts.cortex.as_mut();

    let mut rate_sum = 0;
    msg!("Process resolved_staking_rounds");
    {
        // prints compute budget before
        sol_log_compute_units();
        let resolved_staking_rounds_len_before = cortex.resolved_staking_rounds.len();
        cortex.resolved_staking_rounds.retain_mut(|round| {
            // can the user claim the round rewards
            if stake.stake_time < round.timestamp_start {
                // Add to the rate
                rate_sum += round.rate;
                round.total_claim += stake.amount;
            }
            // retain element if there is stake that has not been claimed yet by other participants
            let round_fully_claimed = round.total_claim == round.total_stake;
            !round_fully_claimed
        });
        let unretained_resolved_staking_rounds_amount =
            resolved_staking_rounds_len_before - cortex.resolved_staking_rounds.len();
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

    msg!("Transfer reward tokens");
    {
        let perpetuals = ctx.accounts.perpetuals.as_mut();
        let rewards_amount = stake.amount * rate_sum;

        // TODO - add a deadline as to when the  caller_reward_token_account is also rewarded with 1% of the stake

        perpetuals.transfer_tokens(
            ctx.accounts.stake_reward_token_account.to_account_info(),
            ctx.accounts.owner.to_account_info(),
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            rewards_amount,
        )?;
    }

    // refresh stake time
    stake.stake_time = ctx.accounts.perpetuals.get_time()?;

    // remove stake from current staking round
    cortex.current_staking_round.total_stake -= stake.amount;

    // add stake to next staking round
    cortex.next_staking_round.total_stake += stake.amount;

    Ok(())
}
