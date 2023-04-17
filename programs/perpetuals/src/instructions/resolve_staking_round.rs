//! ResolveStakingRound instruction handler

use num::Zero;

use crate::{error::PerpetualsError, state::cortex::StakingRound};

use {
    crate::{
        math,
        state::{cortex::Cortex, perpetuals::Perpetuals},
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
};

#[derive(Accounts)]
pub struct ResolveStakingRound<'info> {
    #[account(mut)]
    pub caller: Signer<'info>,

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
        seeds = [b"cortex"],
        bump = cortex.bump,
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

pub fn resolve_staking_round(ctx: Context<ResolveStakingRound>) -> Result<()> {
    let cortex = &mut ctx.accounts.cortex.as_mut();

    // verify that the current round is eligible for resolution
    require!(
        cortex.current_staking_round_is_resolvable(ctx.accounts.perpetuals.get_time()?)?,
        PerpetualsError::InvalidStakingRoundState
    );

    msg!("Calculate current round's rate");
    let current_round_reward_token_amount = math::checked_sub(
        ctx.accounts.stake_reward_token_account.amount,
        cortex.resolved_reward_token_amount,
    )?;
    let current_round_stake_token_amount = math::checked_sub(
        ctx.accounts.stake_token_account.amount,
        cortex.resolved_stake_token_amount,
    )?;

    msg!("Updates Cortex.current_staking_round data");
    {
        match current_round_stake_token_amount {
            0 => cortex.current_staking_round.rate = 0,
            _ => {
                cortex.current_staking_round.rate = math::checked_div(
                    current_round_reward_token_amount,
                    current_round_stake_token_amount,
                )?
            }
        }
        cortex.current_staking_round.total_stake = current_round_stake_token_amount;
        cortex.current_staking_round.total_claim = u64::MIN;
    }

    msg!("Updates Cortex data");
    {
        // add the round to resolved rounds and update data if there was any stake
        // Note: the rewards will go to the next round stakers if a round finishes without anyone staking
        if !current_round_stake_token_amount.is_zero() {
            require!(
                ctx.accounts.stake_token_account.amount
                    == math::checked_add(
                        cortex.resolved_stake_token_amount,
                        current_round_stake_token_amount
                    )?,
                PerpetualsError::InvalidStakingRoundState
            );
            cortex.resolved_reward_token_amount = ctx.accounts.stake_reward_token_account.amount;

            require!(
                ctx.accounts.stake_reward_token_account.amount
                    == math::checked_add(
                        cortex.resolved_reward_token_amount,
                        current_round_reward_token_amount
                    )?,
                PerpetualsError::InvalidStakingRoundState
            );
            cortex.resolved_reward_token_amount = ctx.accounts.stake_reward_token_account.amount;

            let current_staking_round = cortex.current_staking_round.clone();
            cortex.resolved_staking_rounds.push(current_staking_round);
        }
        cortex.current_staking_round = cortex.next_staking_round.clone();
        cortex.next_staking_round = StakingRound::new(ctx.accounts.perpetuals.get_time()?);
    }

    Ok(())
}
