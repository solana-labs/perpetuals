//! InitStaking instruction handler

use {
    crate::{
        math, program,
        state::{
            cortex::Cortex,
            perpetuals::Perpetuals,
            staking::{Staking, CLOCKWORK_PAYER_PUBKEY, STAKING_THREAD_AUTHORITY_SEED},
        },
    },
    anchor_lang::{prelude::*, InstructionData},
    anchor_spl::token::{Mint, Token, TokenAccount},
    solana_program::{instruction::Instruction, program_error::ProgramError, sysvar::clock},
    std::str::FromStr,
};

#[derive(Accounts)]
pub struct InitStaking<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

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

    #[account(
        init,
        payer = owner,
        space = Staking::LEN,
        seeds = [b"staking",
                 owner.key().as_ref()],
        bump
    )]
    pub staking: Box<Account<'info, Staking>>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    /// CHECK: empty PDA, will be set as authority for clockwork threads
    #[account(
        init,
        payer = owner,
        space = 0,
        seeds = [STAKING_THREAD_AUTHORITY_SEED, owner.key().as_ref()],
        bump
    )]
    pub staking_thread_authority: AccountInfo<'info>,

    /// CHECK: checked by clockwork thread program
    #[account(mut)]
    pub stakes_claim_cron_thread: UncheckedAccount<'info>,

    /// CHECK: must match clockwork PAYER_PUBKEY account
    #[account(mut, address = Pubkey::from_str(CLOCKWORK_PAYER_PUBKEY).unwrap())]
    pub stakes_claim_payer: AccountInfo<'info>,

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

    #[account()]
    pub stake_reward_token_mint: Box<Account<'info, Mint>>,

    perpetuals_program: Program<'info, program::Perpetuals>,
    clockwork_program: Program<'info, clockwork_sdk::ThreadProgram>,
    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone)]
pub struct InitStakingParams {
    pub stakes_claim_cron_thread_id: u64,
}

pub fn init_staking(ctx: Context<InitStaking>, params: &InitStakingParams) -> Result<()> {
    let staking = ctx.accounts.staking.as_mut();

    staking.bump = *ctx.bumps.get("staking").ok_or(ProgramError::InvalidSeeds)?;
    staking.thread_authority_bump = *ctx
        .bumps
        .get("staking_thread_authority")
        .ok_or(ProgramError::InvalidSeeds)?;

    staking.locked_stakes = Vec::new();
    staking.stakes_claim_cron_thread_id = params.stakes_claim_cron_thread_id;

    let current_time = clock::Clock::get()?.unix_timestamp;

    // Setup auto-claim cron and pause it (will resume once user stake tokens)
    {
        clockwork_sdk::cpi::thread_create(
            CpiContext::new_with_signer(
                ctx.accounts.clockwork_program.to_account_info(),
                clockwork_sdk::cpi::ThreadCreate {
                    payer: ctx.accounts.owner.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    thread: ctx.accounts.stakes_claim_cron_thread.to_account_info(),
                    authority: ctx.accounts.staking_thread_authority.to_account_info(),
                },
                &[&[
                    STAKING_THREAD_AUTHORITY_SEED,
                    ctx.accounts.owner.key().as_ref(),
                    &[staking.thread_authority_bump],
                ]],
            ),
            // Lamports paid to the clockwork worker executing the thread
            math::checked_add(
                math::checked_mul(
                    Staking::AUTO_CLAIM_FEE_COVERED_CALLS,
                    Staking::AUTOMATION_EXEC_FEE,
                )?,
                // Provide enough for the thread account to be rent exempt
                Rent::get()?.minimum_balance(ctx.accounts.stakes_claim_cron_thread.data_len()),
            )?,
            params.stakes_claim_cron_thread_id.try_to_vec().unwrap(),
            //
            // Instruction to be executed with the thread
            vec![Instruction {
                program_id: crate::ID,
                accounts: crate::cpi::accounts::ClaimStakes {
                    caller: ctx.accounts.stakes_claim_cron_thread.to_account_info(),
                    payer: ctx.accounts.stakes_claim_payer.to_account_info(),
                    owner: ctx.accounts.owner.to_account_info(),
                    owner_reward_token_account: ctx
                        .accounts
                        .owner_reward_token_account
                        .to_account_info(),
                    stake_reward_token_account: ctx
                        .accounts
                        .stake_reward_token_account
                        .to_account_info(),
                    transfer_authority: ctx.accounts.transfer_authority.to_account_info(),
                    staking: staking.to_account_info(),
                    cortex: ctx.accounts.cortex.to_account_info(),
                    perpetuals: ctx.accounts.perpetuals.to_account_info(),
                    stake_reward_token_mint: ctx.accounts.stake_reward_token_mint.to_account_info(),
                    perpetuals_program: ctx.accounts.perpetuals_program.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                }
                .to_account_metas(Some(true)),
                data: crate::instruction::ClaimStakes {}.data(),
            }
            .into()],
            //
            // Trigger configuration
            clockwork_sdk::state::Trigger::Cron {
                // Target format example:
                //   0       0      23    */18   *      *           *
                // seconds minute  hour   days  month  day (week)  year
                //
                // Execute every 18 days at 11pm
                schedule: format!(
                    "{} {} {} */{} * * *",
                    // Use random minute and hour to avoid clockwork worker overload
                    current_time % 60,
                    current_time % 60,
                    current_time % 24,
                    Staking::AUTO_CLAIM_CRON_DAYS_PERIODICITY
                )
                .into(),
                skippable: false,
            },
        )?;

        clockwork_sdk::cpi::thread_pause(CpiContext::new_with_signer(
            ctx.accounts.clockwork_program.to_account_info(),
            clockwork_sdk::cpi::ThreadPause {
                authority: ctx.accounts.staking_thread_authority.to_account_info(),
                thread: ctx.accounts.stakes_claim_cron_thread.to_account_info(),
            },
            &[&[
                STAKING_THREAD_AUTHORITY_SEED,
                ctx.accounts.owner.key().as_ref(),
                &[ctx.accounts.staking.thread_authority_bump],
            ]],
        ))?;
    }

    Ok(())
}
