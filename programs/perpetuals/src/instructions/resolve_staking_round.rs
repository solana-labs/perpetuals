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
        seeds = [b"staking_token_account"],
        bump = cortex.staking_token_account_bump
    )]
    pub staking_token_account: Box<Account<'info, TokenAccount>>,

    // staking reward token vault
    #[account(
        mut,
        token::mint = staking_reward_token_mint,
        seeds = [b"staking_reward_token_account"],
        bump = cortex.staking_reward_token_account_bump
    )]
    pub staking_reward_token_account: Box<Account<'info, TokenAccount>>,

    // staking lm reward token vault
    #[account(
        mut,
        token::mint = lm_token_mint,
        seeds = [b"staking_lm_reward_token_account"],
        bump = cortex.staking_lm_reward_token_account_bump
    )]
    pub staking_lm_reward_token_account: Box<Account<'info, TokenAccount>>,

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
    pub staking_reward_token_mint: Box<Account<'info, Mint>>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

// Note: the rewards will go to the next round stakers if a round finishes without anyone staking
pub fn resolve_staking_round(ctx: Context<ResolveStakingRound>) -> Result<()> {
    let cortex = &mut ctx.accounts.cortex;

    // verify that the current round is eligible for resolution
    require!(
        cortex.current_staking_round_is_resolvable(ctx.accounts.perpetuals.get_time()?)?,
        PerpetualsError::InvalidStakingRoundState
    );

    // Calculate and mint LM token rewards for current round
    {
        // @TODO calculate this one based on emission formula
        let current_round_lm_reward_token_amount = 1_000_000;

        ctx.accounts.perpetuals.mint_tokens(
            ctx.accounts.lm_token_mint.to_account_info(),
            ctx.accounts
                .staking_lm_reward_token_account
                .to_account_info(),
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            current_round_lm_reward_token_amount,
        )?;

        // reload account to account for newly minted tokens
        ctx.accounts.staking_lm_reward_token_account.reload()?;
    }

    // Calculate metrics
    let (
        current_round_reward_token_amount,
        current_round_stake_token_amount,
        current_round_lm_reward_token_amount,
        current_round_lm_stake_token_amount,
    ) = {
        // Consider as reward everything that is in the vault, minus what is already assigned as reward
        let current_round_reward_token_amount = math::checked_sub(
            ctx.accounts.staking_reward_token_account.amount,
            cortex.resolved_reward_token_amount,
        )?;

        let current_round_stake_token_amount = cortex.current_staking_round.total_stake;

        // Consider as reward everything that is in the vault, minus what is already assigned as reward
        let current_round_lm_reward_token_amount = math::checked_sub(
            ctx.accounts.staking_lm_reward_token_account.amount,
            cortex.resolved_lm_reward_token_amount,
        )?;

        let current_round_lm_stake_token_amount = cortex.current_staking_round.lm_total_stake;

        (
            current_round_reward_token_amount,
            current_round_stake_token_amount,
            current_round_lm_reward_token_amount,
            current_round_lm_stake_token_amount,
        )
    };

    // Calculate rates
    {
        // rate
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

        msg!("current round rate {}", cortex.current_staking_round.rate);

        // lm rate
        match current_round_lm_stake_token_amount {
            0 => cortex.current_staking_round.lm_rate = 0,
            _ => {
                cortex.current_staking_round.lm_rate = math::checked_decimal_div(
                    current_round_lm_reward_token_amount,
                    -(Cortex::LM_DECIMALS as i32),
                    current_round_lm_stake_token_amount,
                    -(Cortex::LM_DECIMALS as i32),
                    -(Perpetuals::RATE_DECIMALS as i32),
                )?
            }
        }

        msg!(
            "current round lm rate {}",
            cortex.current_staking_round.lm_rate
        );
    }

    // If there are staked tokens and there are tokens to distribute
    if (!current_round_stake_token_amount.is_zero()
        || !current_round_lm_stake_token_amount.is_zero())
        && (cortex.current_staking_round.rate != 0 || cortex.current_staking_round.lm_rate != 0)
    {
        // Update cortex data
        {
            {
                cortex.resolved_stake_token_amount = math::checked_add(
                    cortex.resolved_stake_token_amount,
                    current_round_stake_token_amount as u128,
                )?;

                require!(
                    ctx.accounts.staking_reward_token_account.amount
                        == math::checked_add(
                            cortex.resolved_reward_token_amount,
                            current_round_reward_token_amount
                        )?,
                    PerpetualsError::InvalidStakingRoundState
                );

                cortex.resolved_reward_token_amount =
                    ctx.accounts.staking_reward_token_account.amount;
            }

            {
                cortex.resolved_lm_stake_token_amount = math::checked_add(
                    cortex.resolved_lm_stake_token_amount,
                    current_round_lm_stake_token_amount as u128,
                )?;

                require!(
                    ctx.accounts.staking_lm_reward_token_account.amount
                        == math::checked_add(
                            cortex.resolved_lm_reward_token_amount,
                            current_round_lm_reward_token_amount
                        )?,
                    PerpetualsError::InvalidStakingRoundState
                );

                cortex.resolved_lm_reward_token_amount =
                    ctx.accounts.staking_lm_reward_token_account.amount;
            }
        }

        // Move current round to resolved rounds array
        {
            let current_staking_round = cortex.current_staking_round.clone();

            cortex.resolved_staking_rounds.push(current_staking_round);
        }

        // Resize cortex account to adapt to resolved_staking_rounds size
        {
            // Safety mesure
            // If too many resolved staking rounds, drop the oldest
            // Should never happens as cron should auto-claim on behalf of users, cleaning resolved rounds on the way
            // Hovever if it does happens, rewards will be redirected to current round (implicit)
            if cortex.resolved_staking_rounds.len() > StakingRound::MAX_RESOLVED_ROUNDS {
                let oldest_round = cortex.resolved_staking_rounds.first().unwrap();

                msg!(
                    "MAX_RESOLVED_ROUNDS ({}) have been reached, drop oldest round",
                    StakingRound::MAX_RESOLVED_ROUNDS
                );

                // Remove round from accounting
                {
                    let stake_token_elligible_to_rewards =
                        math::checked_sub(oldest_round.total_stake, oldest_round.total_claim)?;

                    let unclaimed_rewards = math::checked_decimal_mul(
                        oldest_round.rate,
                        -(Perpetuals::RATE_DECIMALS as i32),
                        stake_token_elligible_to_rewards,
                        -(cortex.stake_token_decimals as i32),
                        -(cortex.stake_reward_token_decimals as i32),
                    )?;

                    cortex.resolved_reward_token_amount =
                        math::checked_sub(cortex.resolved_reward_token_amount, unclaimed_rewards)?;

                    cortex.resolved_stake_token_amount = math::checked_sub(
                        cortex.resolved_stake_token_amount,
                        stake_token_elligible_to_rewards as u128,
                    )?;
                }

                // Delete the round from array
                cortex.resolved_staking_rounds.remove(0);
            } else {
                msg!("realloc cortex size to accomodate for the newly added round");
                {
                    // realloc Cortex after update to its `stake_rounds` if needed
                    Perpetuals::realloc(
                        ctx.accounts.caller.to_account_info(),
                        cortex.to_account_info(),
                        ctx.accounts.system_program.to_account_info(),
                        cortex.size(),
                        true,
                    )?;
                }
            }
        }
    }

    // Now that current round got resolved, setup the new current round
    {
        // replace the current_round with the next_round
        cortex.current_staking_round = cortex.next_staking_round.clone();
        cortex.current_staking_round.start_time = ctx.accounts.perpetuals.get_time()?;
    }

    // Generate new next round
    {
        cortex.next_staking_round = StakingRound {
            start_time: 0,
            rate: u64::MIN,
            total_stake: cortex.next_staking_round.total_stake,
            total_claim: u64::MIN,
            lm_rate: u64::MIN,
            lm_total_stake: cortex.next_staking_round.lm_total_stake,
            lm_total_claim: u64::MIN,
        };
    }

    msg!(
        "cortex.resolved_staking_rounds after {:?} / {}",
        cortex.resolved_staking_rounds,
        cortex.resolved_staking_rounds.len()
    );
    msg!(
        "cortex.current_staking_round after {:?}",
        cortex.current_staking_round
    );
    msg!(
        "cortex.next_staking_round after {:?}",
        cortex.next_staking_round
    );

    Ok(())
}
