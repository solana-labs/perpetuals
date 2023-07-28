//! AddLockedStake instruction handler

use {
    crate::{
        adapters::SplGovernanceV3Adapter,
        instructions::FinalizeLockedStakeParams,
        math, program,
        state::{
            cortex::Cortex,
            perpetuals::Perpetuals,
            staking::{Staking, StakingType},
            user_staking::{LockedStake, UserStaking, USER_STAKING_THREAD_AUTHORITY_SEED},
        },
    },
    anchor_lang::{prelude::*, InstructionData},
    anchor_spl::token::{Mint, Token, TokenAccount},
    solana_program::{instruction::Instruction, program_error::ProgramError},
};

#[derive(Accounts)]
pub struct AddLockedStake<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        token::mint = staking.staked_token_mint,
        has_one = owner
    )]
    pub funding_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = staking_reward_token_mint,
        has_one = owner
    )]
    pub reward_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = staking.staked_token_mint,
        token::authority = transfer_authority,
        seeds = [b"staking_staked_token_vault", staking.key().as_ref()],
        bump = staking.staked_token_vault_bump,
    )]
    pub staking_staked_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = staking_reward_token_mint,
        seeds = [b"staking_reward_token_vault", staking.key().as_ref()],
        bump = staking.reward_token_vault_bump
    )]
    pub staking_reward_token_vault: Box<Account<'info, TokenAccount>>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"user_staking",
                 owner.key().as_ref(), staking.key().as_ref()],
        bump = user_staking.bump
    )]
    pub user_staking: Box<Account<'info, UserStaking>>,

    #[account(
        mut,
        seeds = [b"staking", staking.staked_token_mint.as_ref()],
        bump = staking.bump,
        constraint = staking.reward_token_mint.key() == staking_reward_token_mint.key()
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

    #[account(
        mut,
        seeds = [b"governance_token_mint"],
        bump = cortex.governance_token_bump
    )]
    pub governance_token_mint: Box<Account<'info, Mint>>,

    #[account()]
    pub staking_reward_token_mint: Box<Account<'info, Mint>>,

    /// CHECK: checked by spl governance v3 program
    /// A realm represent one project (ADRENA, MANGO etc.) within the governance program
    pub governance_realm: UncheckedAccount<'info>,

    /// CHECK: checked by spl governance v3 program
    pub governance_realm_config: UncheckedAccount<'info>,

    /// CHECK: checked by spl governance v3 program
    /// Token account owned by governance program holding user's locked tokens
    #[account(mut)]
    pub governance_governing_token_holding: UncheckedAccount<'info>,

    /// CHECK: checked by spl governance v3 program
    /// Account owned by governance storing user informations
    #[account(mut)]
    pub governance_governing_token_owner_record: UncheckedAccount<'info>,

    /// CHECK: checked by clockwork thread program
    #[account(mut)]
    pub stake_resolution_thread: UncheckedAccount<'info>,

    /// CHECK: checked by clockwork thread program
    #[account(mut)]
    pub stakes_claim_cron_thread: Box<Account<'info, clockwork_sdk::state::Thread>>,

    /// CHECK: empty PDA, authority for threads
    #[account(
        seeds = [USER_STAKING_THREAD_AUTHORITY_SEED, user_staking.key().as_ref()],
        bump = user_staking.thread_authority_bump
    )]
    pub user_staking_thread_authority: AccountInfo<'info>,

    clockwork_program: Program<'info, clockwork_sdk::ThreadProgram>,
    governance_program: Program<'info, SplGovernanceV3Adapter>,
    perpetuals_program: Program<'info, program::Perpetuals>,
    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone)]
pub struct AddLockedStakeParams {
    pub stake_resolution_thread_id: u64,

    pub amount: u64,

    // Amount of days to be locked for
    pub locked_days: u32,
}

pub fn add_locked_stake(ctx: Context<AddLockedStake>, params: &AddLockedStakeParams) -> Result<()> {
    let staking_option = {
        msg!("Validate inputs");

        if params.amount == 0 {
            return Err(ProgramError::InvalidArgument.into());
        }

        ctx.accounts
            .user_staking
            .get_locked_staking_option(params.locked_days, ctx.accounts.staking.staking_type)
    }?;

    let staking = ctx.accounts.staking.as_mut();
    let user_staking = ctx.accounts.user_staking.as_mut();
    let perpetuals = ctx.accounts.perpetuals.as_mut();
    let cortex = ctx.accounts.cortex.as_mut();

    // Add stake to UserStaking account
    let (stake_amount_with_reward_multiplier, stake_amount_with_lm_reward_multiplier) = {
        let stake_amount_with_reward_multiplier = math::checked_as_u64(math::checked_div(
            math::checked_mul(params.amount, staking_option.reward_multiplier as u64)? as u128,
            Perpetuals::BPS_POWER,
        )?)?;

        let stake_amount_with_lm_reward_multiplier = math::checked_as_u64(math::checked_div(
            math::checked_mul(params.amount, staking_option.lm_reward_multiplier as u64)? as u128,
            Perpetuals::BPS_POWER,
        )?)?;

        // Add the new locked staking to the list
        user_staking.locked_stakes.push(LockedStake {
            amount: params.amount,
            stake_time: perpetuals.get_time()?,
            claim_time: 0,

            // Transform days in seconds here
            lock_duration: math::checked_mul(staking_option.locked_days as u64, 3_600 * 24)?,
            reward_multiplier: staking_option.reward_multiplier,
            lm_reward_multiplier: staking_option.lm_reward_multiplier,
            vote_multiplier: staking_option.vote_multiplier,

            amount_with_reward_multiplier: stake_amount_with_reward_multiplier,
            amount_with_lm_reward_multiplier: stake_amount_with_lm_reward_multiplier,

            resolved: false,
            stake_resolution_thread_id: params.stake_resolution_thread_id,
        });

        // Adapt the size of the staking account
        Perpetuals::realloc(
            ctx.accounts.owner.to_account_info(),
            user_staking.clone().to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            user_staking.size(),
            false,
        )?;

        // Create a clockwork thread to auto-resolve the staking when it ends
        {
            clockwork_sdk::cpi::thread_create(
                CpiContext::new_with_signer(
                    ctx.accounts.clockwork_program.to_account_info(),
                    clockwork_sdk::cpi::ThreadCreate {
                        payer: ctx.accounts.owner.to_account_info(),
                        system_program: ctx.accounts.system_program.to_account_info(),
                        thread: ctx.accounts.stake_resolution_thread.to_account_info(),
                        authority: ctx.accounts.user_staking_thread_authority.to_account_info(),
                    },
                    &[&[
                        USER_STAKING_THREAD_AUTHORITY_SEED,
                        user_staking.key().as_ref(),
                        &[user_staking.thread_authority_bump],
                    ]],
                ),
                // Lamports paid to the clockwork worker executing the thread
                math::checked_add(
                    UserStaking::AUTOMATION_EXEC_FEE,
                    // Provide enough for the thread account to be rent exempt
                    Rent::get()?.minimum_balance(ctx.accounts.stake_resolution_thread.data_len()),
                )?,
                params.stake_resolution_thread_id.try_to_vec().unwrap(),
                //
                // Instruction to be executed with the thread
                vec![Instruction {
                    program_id: crate::ID,
                    accounts: crate::cpi::accounts::FinalizeLockedStake {
                        caller: ctx.accounts.stake_resolution_thread.to_account_info(),
                        owner: ctx.accounts.owner.to_account_info(),
                        transfer_authority: ctx.accounts.transfer_authority.to_account_info(),
                        user_staking: user_staking.to_account_info(),
                        staking: staking.to_account_info(),
                        cortex: cortex.to_account_info(),
                        perpetuals: perpetuals.to_account_info(),
                        lm_token_mint: ctx.accounts.lm_token_mint.to_account_info(),
                        governance_token_mint: ctx.accounts.governance_token_mint.to_account_info(),
                        governance_realm: ctx.accounts.governance_realm.to_account_info(),
                        governance_realm_config: ctx
                            .accounts
                            .governance_realm_config
                            .to_account_info(),
                        governance_governing_token_holding: ctx
                            .accounts
                            .governance_governing_token_holding
                            .to_account_info(),
                        governance_governing_token_owner_record: ctx
                            .accounts
                            .governance_governing_token_owner_record
                            .to_account_info(),
                        governance_program: ctx.accounts.governance_program.to_account_info(),
                        perpetuals_program: ctx.accounts.perpetuals_program.to_account_info(),
                        system_program: ctx.accounts.system_program.to_account_info(),
                        token_program: ctx.accounts.token_program.to_account_info(),
                    }
                    .to_account_metas(Some(true)),
                    data: crate::instruction::FinalizeLockedStake {
                        params: FinalizeLockedStakeParams {
                            thread_id: params.stake_resolution_thread_id,
                        },
                    }
                    .data(),
                }
                .into()],
                //
                // Trigger configuration
                clockwork_sdk::state::Trigger::Timestamp {
                    unix_ts: staking_option.calculate_end_of_staking(perpetuals.get_time()?)?,
                },
            )?;
        }

        (
            stake_amount_with_reward_multiplier,
            stake_amount_with_lm_reward_multiplier,
        )
    };

    // transfer newly staked tokens to Stake PDA
    msg!("Transfer tokens");
    {
        perpetuals.transfer_tokens_from_user(
            ctx.accounts.funding_account.to_account_info(),
            ctx.accounts.staking_staked_token_vault.to_account_info(),
            ctx.accounts.owner.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            params.amount,
        )?;
    }

    //
    //           LM Staking
    //   ---------------------------
    //   voting power         | x1 |
    //   real yield rewards   | x1 |
    //   lm rewards           | x1 |
    //   ---------------------------
    //
    //           LP Staking
    //   ---------------------------
    //   voting power         |  0 |
    //   real yield rewards   | x1 |
    //   lm rewards           | x1 |
    //   ---------------------------
    //
    //
    {
        if staking.staking_type == StakingType::LM {
            // Give governing power to the Stake owner
            {
                // Apply voting multiplier related to locking period
                let voting_power = math::checked_as_u64(math::checked_div(
                    math::checked_mul(params.amount, staking_option.vote_multiplier as u64)?
                        as u128,
                    Perpetuals::BPS_POWER,
                )?)?;

                perpetuals.add_governing_power(
                    ctx.accounts.transfer_authority.to_account_info(),
                    ctx.accounts.owner.to_account_info(),
                    ctx.accounts.owner.to_account_info(),
                    ctx.accounts
                        .governance_governing_token_owner_record
                        .to_account_info(),
                    ctx.accounts.governance_token_mint.to_account_info(),
                    ctx.accounts.governance_realm.to_account_info(),
                    ctx.accounts.governance_realm_config.to_account_info(),
                    ctx.accounts
                        .governance_governing_token_holding
                        .to_account_info(),
                    ctx.accounts.governance_program.to_account_info(),
                    voting_power,
                    None,
                    true,
                )?;
            }
        }

        staking.next_staking_round.total_stake = math::checked_add(
            staking.next_staking_round.total_stake,
            stake_amount_with_reward_multiplier,
        )?;

        staking.next_staking_round.lm_total_stake = math::checked_add(
            staking.next_staking_round.lm_total_stake,
            stake_amount_with_lm_reward_multiplier,
        )?;

        staking.nb_locked_tokens = math::checked_add(staking.nb_locked_tokens, params.amount)?;
    }

    // If auto claim thread is paused, resume it
    {
        if ctx.accounts.stakes_claim_cron_thread.paused {
            clockwork_sdk::cpi::thread_resume(CpiContext::new_with_signer(
                ctx.accounts.clockwork_program.to_account_info(),
                clockwork_sdk::cpi::ThreadResume {
                    authority: ctx.accounts.user_staking_thread_authority.to_account_info(),
                    thread: ctx.accounts.stakes_claim_cron_thread.to_account_info(),
                },
                &[&[
                    USER_STAKING_THREAD_AUTHORITY_SEED,
                    user_staking.key().as_ref(),
                    &[ctx.accounts.user_staking.thread_authority_bump],
                ]],
            ))?;
        }
    }

    Ok(())
}
