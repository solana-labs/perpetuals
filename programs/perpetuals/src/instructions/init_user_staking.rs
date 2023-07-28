//! InitUserStaking instruction handler

use {
    crate::{
        math, program,
        state::{
            cortex::Cortex,
            perpetuals::Perpetuals,
            staking::Staking,
            user_staking::{
                UserStaking, CLOCKWORK_PAYER_PUBKEY, USER_STAKING_THREAD_AUTHORITY_SEED,
            },
        },
    },
    anchor_lang::{prelude::*, InstructionData},
    anchor_spl::token::{Mint, Token, TokenAccount},
    solana_program::{instruction::Instruction, program_error::ProgramError, sysvar::clock},
    std::str::FromStr,
};

#[derive(Accounts)]
pub struct InitUserStaking<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    // reward token account of the stake owner
    #[account(
        mut,
        token::mint = staking_reward_token_mint,
        has_one = owner
    )]
    pub reward_token_account: Box<Account<'info, TokenAccount>>,

    // reward token account of the stake owner
    #[account(
        mut,
        token::mint = lm_token_mint,
        has_one = owner
    )]
    pub lm_token_account: Box<Account<'info, TokenAccount>>,

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

    #[account(
        init,
        payer = owner,
        space = UserStaking::LEN,
        seeds = [b"user_staking",
                 owner.key().as_ref(), staking.key().as_ref()],
        bump
    )]
    pub user_staking: Box<Account<'info, UserStaking>>,

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
        seeds = [USER_STAKING_THREAD_AUTHORITY_SEED, user_staking.key().as_ref()],
        bump
    )]
    pub user_staking_thread_authority: AccountInfo<'info>,

    /// CHECK: checked by clockwork thread program
    #[account(mut)]
    pub stakes_claim_cron_thread: UncheckedAccount<'info>,

    /// CHECK: must match clockwork PAYER_PUBKEY account
    #[account(mut, address = Pubkey::from_str(CLOCKWORK_PAYER_PUBKEY).unwrap())]
    pub stakes_claim_payer: AccountInfo<'info>,

    #[account(
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
        seeds = [b"lm_token_mint"],
        bump = cortex.lm_token_bump
    )]
    pub lm_token_mint: Box<Account<'info, Mint>>,

    #[account()]
    pub staking_reward_token_mint: Box<Account<'info, Mint>>,

    perpetuals_program: Program<'info, program::Perpetuals>,
    clockwork_program: Program<'info, clockwork_sdk::ThreadProgram>,
    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone)]
pub struct InitUserStakingParams {
    pub stakes_claim_cron_thread_id: u64,
}

pub fn init_user_staking(
    ctx: Context<InitUserStaking>,
    params: &InitUserStakingParams,
) -> Result<()> {
    let user_staking = ctx.accounts.user_staking.as_mut();

    user_staking.bump = *ctx
        .bumps
        .get("user_staking")
        .ok_or(ProgramError::InvalidSeeds)?;
    user_staking.thread_authority_bump = *ctx
        .bumps
        .get("user_staking_thread_authority")
        .ok_or(ProgramError::InvalidSeeds)?;

    user_staking.locked_stakes = Vec::new();
    user_staking.stakes_claim_cron_thread_id = params.stakes_claim_cron_thread_id;

    user_staking.liquid_stake.amount = u64::MIN;
    user_staking.liquid_stake.stake_time = 0;
    user_staking.liquid_stake.claim_time = 0;
    user_staking.liquid_stake.overlap_time = 0;
    user_staking.liquid_stake.overlap_amount = u64::MIN;

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
                math::checked_mul(
                    UserStaking::AUTO_CLAIM_FEE_COVERED_CALLS,
                    UserStaking::AUTOMATION_EXEC_FEE,
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
                    reward_token_account: ctx.accounts.reward_token_account.to_account_info(),
                    lm_token_account: ctx.accounts.lm_token_account.to_account_info(),
                    staking_reward_token_vault: ctx
                        .accounts
                        .staking_reward_token_vault
                        .to_account_info(),
                    staking_lm_reward_token_vault: ctx
                        .accounts
                        .staking_lm_reward_token_vault
                        .to_account_info(),
                    transfer_authority: ctx.accounts.transfer_authority.to_account_info(),
                    user_staking: user_staking.to_account_info(),
                    staking: ctx.accounts.staking.to_account_info(),
                    cortex: ctx.accounts.cortex.to_account_info(),
                    perpetuals: ctx.accounts.perpetuals.to_account_info(),
                    staking_reward_token_mint: ctx
                        .accounts
                        .staking_reward_token_mint
                        .to_account_info(),
                    lm_token_mint: ctx.accounts.lm_token_mint.to_account_info(),
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
                    UserStaking::AUTO_CLAIM_CRON_DAYS_PERIODICITY
                ),
                skippable: false,
            },
        )?;

        clockwork_sdk::cpi::thread_pause(CpiContext::new_with_signer(
            ctx.accounts.clockwork_program.to_account_info(),
            clockwork_sdk::cpi::ThreadPause {
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

    Ok(())
}
