//! ResolveStakingRound instruction handler

use {
    crate::{
        error::PerpetualsError,
        instructions::{BucketName, MintLmTokensFromBucketParams},
        math,
        state::{
            cortex::Cortex,
            perpetuals::Perpetuals,
            staking::{Staking, StakingRound},
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

    #[account(
        mut,
        token::mint = staking.staked_token_mint,
        seeds = [b"staking_staked_token_vault", staking.key().as_ref()],
        bump = staking.staked_token_vault_bump
    )]
    pub staking_staked_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = staking_reward_token_mint,
        seeds = [b"staking_reward_token_vault", staking.key().as_ref()],
        bump = staking.reward_token_vault_bump
    )]
    pub staking_reward_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = lm_token_mint,
        seeds = [b"staking_lm_reward_token_vault", staking.key().as_ref()],
        bump = staking.lm_reward_token_vault_bump
    )]
    pub staking_lm_reward_token_vault: Box<Account<'info, TokenAccount>>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"staking", staking.staked_token_mint.as_ref()],
        bump = staking.bump,
    )]
    pub staking: Box<Account<'info, Staking>>,

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

    pub perpetuals_program: Program<'info, Perpetuals>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}

// Note: the rewards will go to the next round stakers if a round finishes without anyone staking
pub fn resolve_staking_round(ctx: Context<ResolveStakingRound>) -> Result<()> {
    let staking = &mut ctx.accounts.staking;

    // verify that the current round is eligible for resolution
    require!(
        staking.current_staking_round_is_resolvable(ctx.accounts.perpetuals.get_time()?)?,
        PerpetualsError::InvalidStakingRoundState
    );

    // Calculate and mint LM token rewards for current round
    {
        // @TODO calculate this one based on emission formula
        let current_round_lm_reward_token_amount = 1_000_000;

        // Mint LM tokens
        {
            if current_round_lm_reward_token_amount > 0 {
                let cpi_accounts = crate::cpi::accounts::MintLmTokensFromBucket {
                    admin: ctx.accounts.transfer_authority.to_account_info(),
                    receiving_account: ctx.accounts.staking_lm_reward_token_vault.to_account_info(),
                    transfer_authority: ctx.accounts.transfer_authority.to_account_info(),
                    cortex: ctx.accounts.cortex.to_account_info(),
                    perpetuals: ctx.accounts.perpetuals.to_account_info(),
                    lm_token_mint: ctx.accounts.lm_token_mint.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                };

                let cpi_program = ctx.accounts.perpetuals_program.to_account_info();
                crate::cpi::mint_lm_tokens_from_bucket(
                    CpiContext::new_with_signer(
                        cpi_program,
                        cpi_accounts,
                        &[&[
                            b"transfer_authority",
                            &[ctx.accounts.perpetuals.transfer_authority_bump],
                        ]],
                    ),
                    MintLmTokensFromBucketParams {
                        bucket_name: BucketName::Ecosystem,
                        amount: current_round_lm_reward_token_amount,
                        reason: String::from("UserStaking rewards"),
                    },
                )?;

                {
                    ctx.accounts.staking_lm_reward_token_vault.reload()?;
                    ctx.accounts.cortex.reload()?;
                    ctx.accounts.perpetuals.reload()?;
                    ctx.accounts.lm_token_mint.reload()?;
                }
            }
        }

        /*
        ctx.accounts.perpetuals.mint_tokens(
            ctx.accounts.lm_token_mint.to_account_info(),
            ctx.accounts
                .staking_lm_reward_token_account
                .to_account_info(),
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            current_round_lm_reward_token_amount,
        )?;*/

        // reload account to account for newly minted tokens
        ctx.accounts.staking_lm_reward_token_vault.reload()?;
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
            ctx.accounts.staking_reward_token_vault.amount,
            staking.resolved_reward_token_amount,
        )?;

        let current_round_stake_token_amount = staking.current_staking_round.total_stake;

        // Consider as reward everything that is in the vault, minus what is already assigned as reward
        let current_round_lm_reward_token_amount = math::checked_sub(
            ctx.accounts.staking_lm_reward_token_vault.amount,
            staking.resolved_lm_reward_token_amount,
        )?;

        let current_round_lm_stake_token_amount = staking.current_staking_round.lm_total_stake;

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
            0 => staking.current_staking_round.rate = 0,
            _ => {
                staking.current_staking_round.rate = math::checked_decimal_div(
                    current_round_reward_token_amount,
                    -(staking.reward_token_decimals as i32),
                    current_round_stake_token_amount,
                    -(staking.staked_token_decimals as i32),
                    -(Perpetuals::RATE_DECIMALS as i32),
                )?
            }
        }

        msg!("current round rate {}", staking.current_staking_round.rate);

        // lm rate
        match current_round_lm_stake_token_amount {
            0 => staking.current_staking_round.lm_rate = 0,
            _ => {
                staking.current_staking_round.lm_rate = math::checked_decimal_div(
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
            staking.current_staking_round.lm_rate
        );
    }

    // If there are staked tokens and there are tokens to distribute
    if (!current_round_stake_token_amount.is_zero()
        || !current_round_lm_stake_token_amount.is_zero())
        && (staking.current_staking_round.rate != 0 || staking.current_staking_round.lm_rate != 0)
    {
        // Update cortex data
        {
            {
                staking.resolved_staked_token_amount = math::checked_add(
                    staking.resolved_staked_token_amount,
                    current_round_stake_token_amount,
                )?;

                require!(
                    ctx.accounts.staking_reward_token_vault.amount
                        == math::checked_add(
                            staking.resolved_reward_token_amount,
                            current_round_reward_token_amount
                        )?,
                    PerpetualsError::InvalidStakingRoundState
                );

                staking.resolved_reward_token_amount =
                    ctx.accounts.staking_reward_token_vault.amount;
            }

            {
                staking.resolved_lm_staked_token_amount = math::checked_add(
                    staking.resolved_lm_staked_token_amount,
                    current_round_lm_stake_token_amount,
                )?;

                require!(
                    ctx.accounts.staking_lm_reward_token_vault.amount
                        == math::checked_add(
                            staking.resolved_lm_reward_token_amount,
                            current_round_lm_reward_token_amount
                        )?,
                    PerpetualsError::InvalidStakingRoundState
                );

                staking.resolved_lm_reward_token_amount =
                    ctx.accounts.staking_lm_reward_token_vault.amount;
            }
        }

        // Move current round to resolved rounds array
        {
            let current_staking_round = staking.current_staking_round.clone();

            staking.resolved_staking_rounds.push(current_staking_round);
        }

        // Resize cortex account to adapt to resolved_staking_rounds size
        {
            // Safety mesure
            // If too many resolved staking rounds, drop the oldest
            // Should never happens as cron should auto-claim on behalf of users, cleaning resolved rounds on the way
            // Hovever if it does happens, rewards will be redirected to current round (implicit)
            if staking.resolved_staking_rounds.len() > StakingRound::MAX_RESOLVED_ROUNDS {
                let oldest_round = staking.resolved_staking_rounds.first().unwrap();

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
                        -(staking.staked_token_decimals as i32),
                        -(staking.reward_token_decimals as i32),
                    )?;

                    staking.resolved_reward_token_amount =
                        math::checked_sub(staking.resolved_reward_token_amount, unclaimed_rewards)?;

                    staking.resolved_staked_token_amount = math::checked_sub(
                        staking.resolved_staked_token_amount,
                        stake_token_elligible_to_rewards,
                    )?;
                }

                // Delete the round from array
                staking.resolved_staking_rounds.remove(0);
            } else {
                msg!("realloc cortex size to accomodate for the newly added round");
                {
                    // realloc Cortex after update to its `stake_rounds` if needed
                    Perpetuals::realloc(
                        ctx.accounts.caller.to_account_info(),
                        staking.to_account_info(),
                        ctx.accounts.system_program.to_account_info(),
                        staking.size(),
                        true,
                    )?;
                }
            }
        }
    }

    // Now that current round got resolved, setup the new current round
    {
        // replace the current_round with the next_round
        staking.current_staking_round = staking.next_staking_round.clone();
        staking.current_staking_round.start_time = ctx.accounts.perpetuals.get_time()?;
    }

    // Generate new next round
    {
        staking.next_staking_round = StakingRound {
            start_time: 0,
            rate: u64::MIN,
            total_stake: staking.next_staking_round.total_stake,
            total_claim: u64::MIN,
            lm_rate: u64::MIN,
            lm_total_stake: staking.next_staking_round.lm_total_stake,
            lm_total_claim: u64::MIN,
        };
    }

    msg!(
        "staking.resolved_staking_rounds after {:?} / {}",
        staking.resolved_staking_rounds,
        staking.resolved_staking_rounds.len()
    );
    msg!(
        "staking.current_staking_round after {:?}",
        staking.current_staking_round
    );
    msg!(
        "staking.next_staking_round after {:?}",
        staking.next_staking_round
    );

    Ok(())
}
