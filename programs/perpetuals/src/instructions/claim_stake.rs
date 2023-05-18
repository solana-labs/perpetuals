//! ClaimStake instruction handler

use {
    crate::{
        math,
        state::{cortex::Cortex, perpetuals::Perpetuals, stake::Stake},
    },
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
    let cortex = ctx.accounts.cortex.as_mut();
    let did_claim: bool;

    // rewards = rate_sum * token_staked -- Done this way as any stake/unstake claim all previous rewards
    let mut rate_sum = 0;
    msg!("Process resolved rounds");
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
            // note: some dust of rewards will build up in the token account due to rate precision of 9 units
            !round_fully_claimed
        });
        let staking_rounds_delta = math::checked_sub(
            cortex.resolved_staking_rounds.len() as i32,
            resolved_staking_rounds_len_before as i32,
        )?;
        // prints compute budget after
        sol_log_compute_units();

        // realloc Cortex after update to its `stake_rounds` if needed
        if !staking_rounds_delta.is_zero() {
            msg!("Realloc Cortex");
            Perpetuals::realloc(
                ctx.accounts.caller.to_account_info(),
                cortex.clone().to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                cortex.new_size(staking_rounds_delta)?,
                true,
            )?;
        }
    }

    msg!("Rewards distribution");
    {
        let reward_token_amount = math::checked_decimal_mul(
            stake.amount,
            -(cortex.stake_token_decimals as i32),
            rate_sum,
            -(Perpetuals::RATE_DECIMALS as i32),
            -(cortex.stake_reward_token_decimals as i32),
        )?;
        if !reward_token_amount.is_zero() {
            msg!("Transfer reward_token_amount: {}", reward_token_amount);
            let perpetuals = ctx.accounts.perpetuals.as_mut();

            let caller_reward_token_amount = stake.get_claim_stake_caller_reward_token_amounts(
                reward_token_amount,
                perpetuals.get_time()?,
            )?;

            let owner_reward_token_amount =
                math::checked_sub(reward_token_amount, caller_reward_token_amount)?;

            msg!("owner_reward_token_amount: {}", owner_reward_token_amount);
            msg!("caller_reward_token_amount: {}", caller_reward_token_amount);

            perpetuals.transfer_tokens(
                ctx.accounts.stake_reward_token_account.to_account_info(),
                ctx.accounts.owner_reward_token_account.to_account_info(),
                ctx.accounts.transfer_authority.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                owner_reward_token_amount,
            )?;

            if !caller_reward_token_amount.is_zero() {
                perpetuals.transfer_tokens(
                    ctx.accounts.stake_reward_token_account.to_account_info(),
                    ctx.accounts.caller_reward_token_account.to_account_info(),
                    ctx.accounts.transfer_authority.to_account_info(),
                    ctx.accounts.token_program.to_account_info(),
                    caller_reward_token_amount,
                )?;
            }

            // refresh stake time while keeping the stake time out of the current round
            // so that the user stay eligible for current round rewards
            stake.stake_time = math::checked_sub(cortex.current_staking_round.start_time, 1)?;

            // remove stake from current staking round
            cortex.current_staking_round.total_stake =
                math::checked_sub(cortex.current_staking_round.total_stake, stake.amount)?;

            // update resolved stake token amount left, by removing the previously staked amount
            cortex.resolved_stake_token_amount =
                math::checked_sub(cortex.resolved_stake_token_amount, stake.amount)?;

            // update resolved reward token amount left
            cortex.resolved_reward_token_amount =
                math::checked_sub(cortex.resolved_reward_token_amount, reward_token_amount)?;

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
    Ok(did_claim)
}
