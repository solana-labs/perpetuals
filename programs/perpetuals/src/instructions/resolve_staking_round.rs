//! ResolveStakingRound instruction handler

use {
    crate::{
        error::PerpetualsError,
        math,
        state::{
            cortex::{Cortex, StakingRound},
            perpetuals::Perpetuals,
        },
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
    num::Zero,
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
    let cortex = &mut ctx.accounts.cortex;

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

    let current_round_stake_token_amount = cortex.current_staking_round.total_stake;

    msg!("reward_token_amount: {}", current_round_reward_token_amount);
    msg!("stake_token_amount: {}", current_round_stake_token_amount);

    msg!("updates Cortex.current_staking_round data");
    {
        match current_round_stake_token_amount {
            0 => cortex.current_staking_round.rate = 0,
            _ => {
                cortex.current_staking_round.rate = math::checked_decimal_div(
                    current_round_reward_token_amount,
                    -(cortex.stake_reward_token_decimals as i32),
                    current_round_stake_token_amount,
                    -(cortex.stake_token_decimals as i32),
                    -(Perpetuals::RATE_DECIMALS as i32),
                )?
            }
        }
        cortex.current_staking_round.total_claim = u64::MIN;
    }
    msg!("rate {}", cortex.current_staking_round.rate);

    msg!("updates Cortex data");
    {
        // add the current round to resolved rounds and update data if there was any stake
        // Note: the rewards will go to the next round stakers if a round finishes without anyone staking
        if !current_round_stake_token_amount.is_zero() {
            /*
            // Not true anymore with multipliers
            require!(
                ctx.accounts.stake_token_account.amount
                    == math::checked_add(
                        cortex.resolved_stake_token_amount,
                        current_round_stake_token_amount
                    )?,
                PerpetualsError::InvalidStakingRoundState
            );*/

            cortex.resolved_stake_token_amount = current_round_stake_token_amount;

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

            msg!("realloc cortex size to accomodate for the newly added round");
            {
                // realloc Cortex after update to its `stake_rounds` if needed
                Perpetuals::realloc(
                    ctx.accounts.caller.to_account_info(),
                    cortex.clone().to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                    cortex.new_size(1)?,
                    true,
                )?;
            }
        }

        // now replace the current_round with the next_round
        let mut new_current_round = cortex.next_staking_round.clone();
        new_current_round.start_time = ctx.accounts.perpetuals.get_time()?;

        // and shift the rounds
        cortex.current_staking_round = new_current_round;
        cortex.next_staking_round = StakingRound {
            start_time: 0,
            rate: u64::MIN,
            total_stake: cortex.next_staking_round.total_stake,
            total_claim: u64::MIN,
        };
    }

    msg!(
        "Cortex.resolved_staking_rounds after {:?}",
        cortex.resolved_staking_rounds
    );
    msg!(
        "Cortex.current_staking_round after {:?}",
        cortex.current_staking_round
    );
    msg!(
        "Cortex.next_staking_round after {:?}",
        cortex.next_staking_round
    );

    Ok(())
}
