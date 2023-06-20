//! ClaimStake instruction handler

use {
    crate::{
        math, program,
        state::{cortex::Cortex, perpetuals::Perpetuals, staking::Staking},
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
    num::Zero,
    solana_program::log::sol_log_compute_units,
};

#[derive(Accounts)]
pub struct ClaimStakes<'info> {
    #[account(mut)]
    pub caller: Signer<'info>,

    // Pay for realloc
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: verified through the `stake` account seed derivation
    #[account(mut)]
    pub owner: AccountInfo<'info>,

    // reward token account of the stake owner
    #[account(
        mut,
        token::mint = stake_reward_token_mint,
        has_one = owner
    )]
    pub owner_reward_token_account: Box<Account<'info, TokenAccount>>,

    // reward token account of the stake owner
    #[account(
        mut,
        token::mint = lm_token_mint,
        has_one = owner
    )]
    pub owner_lm_reward_token_account: Box<Account<'info, TokenAccount>>,

    // staking reward token vault
    #[account(
        mut,
        token::mint = stake_reward_token_mint,
        seeds = [b"stake_reward_token_account"],
        bump = cortex.stake_reward_token_account_bump
    )]
    pub stake_reward_token_account: Box<Account<'info, TokenAccount>>,

    // staking lm reward token vault
    #[account(
        mut,
        token::mint = lm_token_mint,
        seeds = [b"stake_lm_reward_token_account"],
        bump = cortex.stake_lm_reward_token_account_bump
    )]
    pub stake_lm_reward_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"staking",
                 owner.key().as_ref()],
        bump = staking.bump
    )]
    pub staking: Box<Account<'info, Staking>>,

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
        seeds = [b"lm_token_mint"],
        bump = cortex.lm_token_bump
    )]
    pub lm_token_mint: Box<Account<'info, Mint>>,

    #[account()]
    pub stake_reward_token_mint: Box<Account<'info, Mint>>,

    perpetuals_program: Program<'info, program::Perpetuals>,
    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
}

pub fn claim_stakes(ctx: Context<ClaimStakes>) -> Result<()> {
    let staking = ctx.accounts.staking.as_mut();
    let cortex = ctx.accounts.cortex.as_mut();

    msg!("Process resolved rounds & rewards calculation");

    // Loop over resolved rounds and:
    // 1. Calculate rewards token amount to be claimed for staker
    // 2. Drop fully claimed rounds
    let (
        rewards_token_amount,
        stake_amount_with_reward_multiplier,
        lm_rewards_token_amount,
        stake_amount_with_lm_reward_multiplier,
    ) = {
        // prints compute budget before
        sol_log_compute_units();

        let resolved_staking_rounds_len_before = cortex.resolved_staking_rounds.len();
        let stake_token_decimals = cortex.stake_token_decimals as i32;
        let stake_reward_token_decimals = cortex.stake_reward_token_decimals as i32;

        let mut rewards_token_amount: u64 = 0;
        let mut lm_rewards_token_amount: u64 = 0;

        // total amount of token that tokens have been claimed for
        let mut stake_amount_with_reward_multiplier: u64 = 0;
        let mut stake_amount_with_lm_reward_multiplier: u64 = 0;

        msg!(
            "{} resolved rounds to evaluate",
            cortex.resolved_staking_rounds.len()
        );

        // For each resolved staking rounds
        cortex.resolved_staking_rounds.retain_mut(|round| {
            // Calculate rewards for locked stakes
            {
                // For each user locked stakes
                for locked_stake in staking.locked_stakes.iter_mut() {
                    // Stake is elligible for rewards
                    if locked_stake.qualifies_for_rewards_from(round) {
                        {
                            let locked_rewards_token_amount = math::checked_decimal_mul(
                                locked_stake.amount_with_reward_multiplier,
                                -stake_token_decimals,
                                round.rate,
                                -(Perpetuals::RATE_DECIMALS as i32),
                                -stake_reward_token_decimals,
                            )
                            .unwrap();

                            rewards_token_amount = math::checked_add(
                                rewards_token_amount,
                                locked_rewards_token_amount,
                            )
                            .unwrap();

                            round.total_claim = math::checked_add(
                                round.total_claim,
                                locked_stake.amount_with_reward_multiplier,
                            )
                            .unwrap();

                            stake_amount_with_reward_multiplier = math::checked_add(
                                stake_amount_with_reward_multiplier,
                                locked_stake.amount_with_reward_multiplier,
                            )
                            .unwrap();
                        }

                        {
                            let locked_lm_rewards_token_amount = math::checked_decimal_mul(
                                locked_stake.amount_with_lm_reward_multiplier,
                                -(Cortex::LM_DECIMALS as i32),
                                round.lm_rate,
                                -(Perpetuals::RATE_DECIMALS as i32),
                                -(Cortex::LM_DECIMALS as i32),
                            )
                            .unwrap();

                            lm_rewards_token_amount = math::checked_add(
                                lm_rewards_token_amount,
                                locked_lm_rewards_token_amount,
                            )
                            .unwrap();

                            round.lm_total_claim = math::checked_add(
                                round.lm_total_claim,
                                locked_stake.amount_with_lm_reward_multiplier,
                            )
                            .unwrap();

                            stake_amount_with_lm_reward_multiplier = math::checked_add(
                                stake_amount_with_lm_reward_multiplier,
                                locked_stake.amount_with_lm_reward_multiplier,
                            )
                            .unwrap();
                        }
                    }
                }
            }

            // Calculate rewards for liquid stake
            {
                // Stake is elligible for rewards
                if staking.liquid_stake.qualifies_for_rewards_from(round) {
                    let liquid_rewards_token_amount = math::checked_decimal_mul(
                        staking.liquid_stake.amount,
                        -stake_token_decimals,
                        round.rate,
                        -(Perpetuals::RATE_DECIMALS as i32),
                        -stake_reward_token_decimals,
                    )
                    .unwrap();

                    msg!("Liquid stake: Qualifying for rewards :)");

                    rewards_token_amount =
                        math::checked_add(rewards_token_amount, liquid_rewards_token_amount)
                            .unwrap();

                    round.total_claim =
                        math::checked_add(round.total_claim, staking.liquid_stake.amount).unwrap();

                    stake_amount_with_reward_multiplier = math::checked_add(
                        stake_amount_with_reward_multiplier,
                        staking.liquid_stake.amount,
                    )
                    .unwrap();
                } else {
                    msg!("Liquid stake: Not qualifying for rewards :(");
                }
            }

            // retain element if there is stake that has not been claimed yet by other participants
            let round_fully_claimed = round.total_claim == round.total_stake
                && round.lm_total_claim == round.lm_total_stake;

            // note: some dust of rewards will build up in the token account due to rate precision of 9 units
            !round_fully_claimed
        });

        // Realloc Cortex to account for dropped staking rounds if needed
        {
            let staking_rounds_delta = math::checked_sub(
                cortex.resolved_staking_rounds.len() as i32,
                resolved_staking_rounds_len_before as i32,
            )?;

            if !staking_rounds_delta.is_zero() {
                msg!("Realloc Cortex");

                // TODO: ADD PAYER ACCOUNT FOR CLAIM AND USE THE CLOCKWORK DELEGATED PAYER PUBKEY: C1ockworkPayer11111111111111111111111111111
                Perpetuals::realloc(
                    ctx.accounts.payer.to_account_info(),
                    cortex.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                    cortex.size(),
                    true,
                )?;
            }
        }

        // prints compute budget after
        sol_log_compute_units();

        (
            rewards_token_amount,
            stake_amount_with_reward_multiplier,
            lm_rewards_token_amount,
            stake_amount_with_lm_reward_multiplier,
        )
    };

    msg!("Distribute {} rewards", rewards_token_amount);

    {
        if !rewards_token_amount.is_zero() {
            msg!("Transfer rewards_token_amount: {}", rewards_token_amount);

            let perpetuals = ctx.accounts.perpetuals.as_mut();

            perpetuals.transfer_tokens(
                ctx.accounts.stake_reward_token_account.to_account_info(),
                ctx.accounts.owner_reward_token_account.to_account_info(),
                ctx.accounts.transfer_authority.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                rewards_token_amount,
            )?;
        } else {
            msg!("No reward tokens to claim at this time");
        }
    }

    msg!("Distribute {} lm rewards", lm_rewards_token_amount);

    {
        if !lm_rewards_token_amount.is_zero() {
            msg!(
                "Transfer lm_rewards_token_amount: {}",
                lm_rewards_token_amount
            );

            let perpetuals = ctx.accounts.perpetuals.as_mut();

            perpetuals.transfer_tokens(
                ctx.accounts.stake_lm_reward_token_account.to_account_info(),
                ctx.accounts.owner_lm_reward_token_account.to_account_info(),
                ctx.accounts.transfer_authority.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                lm_rewards_token_amount,
            )?;
        } else {
            msg!("No lm reward tokens to claim at this time");
        }
    }

    // Update stakes claim time
    {
        // refresh claim time while keeping the claim time out of the current round
        // so that the user stay eligible for current round rewards
        let claim_time = math::checked_sub(cortex.current_staking_round.start_time, 1)?;

        // Locked staking
        for mut locked_stake in staking.locked_stakes.iter_mut() {
            locked_stake.claim_time = claim_time;
        }

        // Liquid staking
        staking.liquid_stake.claim_time = claim_time;
    }

    // Adapt current/next round
    {
        {
            // update resolved stake token amount left, by removing the previously staked amount
            cortex.resolved_stake_token_amount = math::checked_sub(
                cortex.resolved_stake_token_amount,
                stake_amount_with_reward_multiplier as u128,
            )?;

            // update resolved reward token amount left
            cortex.resolved_reward_token_amount =
                math::checked_sub(cortex.resolved_reward_token_amount, rewards_token_amount)?;
        }

        {
            // update resolved lm stake token amount left, by removing the previously staked amount
            cortex.resolved_lm_stake_token_amount = math::checked_sub(
                cortex.resolved_lm_stake_token_amount,
                stake_amount_with_lm_reward_multiplier as u128,
            )?;

            // update resolved reward token amount left
            cortex.resolved_lm_reward_token_amount = math::checked_sub(
                cortex.resolved_lm_reward_token_amount,
                lm_rewards_token_amount,
            )?;
        }

        msg!(
            "cortex.resolved_reward_token_amount after claim stake {:?}",
            cortex.resolved_reward_token_amount
        );

        msg!(
            "cortex.resolved_lm_reward_token_amount after claim stake {:?}",
            cortex.resolved_lm_reward_token_amount
        );

        msg!(
            "cortex.resolved_staking_rounds after claim stake {:?}",
            cortex.resolved_staking_rounds
        );
    }

    Ok(())
}
