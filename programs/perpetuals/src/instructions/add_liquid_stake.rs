//! AddLiquidStake instruction handler

use {
    crate::{
        adapters::SplGovernanceV3Adapter,
        math, program,
        state::{
            cortex::Cortex,
            perpetuals::Perpetuals,
            staking::{Staking, StakingType},
            user_staking::{UserStaking, USER_STAKING_THREAD_AUTHORITY_SEED},
        },
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
    solana_program::program_error::ProgramError,
};

#[derive(Accounts)]
pub struct AddLiquidStake<'info> {
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

    // reward token account of the stake owner
    #[account(
        mut,
        token::mint = lm_token_mint,
        has_one = owner
    )]
    pub lm_token_account: Box<Account<'info, TokenAccount>>,

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
pub struct AddLiquidStakeParams {
    pub amount: u64,
}

pub fn add_liquid_stake(ctx: Context<AddLiquidStake>, params: &AddLiquidStakeParams) -> Result<()> {
    {
        msg!("Validate inputs");

        if params.amount == 0 {
            return Err(ProgramError::InvalidArgument.into());
        }
    }

    let staking = ctx.accounts.staking.as_mut();
    let user_staking = ctx.accounts.user_staking.as_mut();
    let perpetuals = ctx.accounts.perpetuals.as_mut();
    let cortex = ctx.accounts.cortex.as_mut();

    // Add stake to UserStaking account
    {
        // If liquid staking is already ongoing
        if user_staking.liquid_stake.amount > 0 {
            // Claim rewards
            {
                let cpi_accounts = crate::cpi::accounts::ClaimStakes {
                    caller: ctx.accounts.owner.to_account_info(),
                    payer: ctx.accounts.owner.to_account_info(),
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
                    staking: staking.to_account_info(),
                    cortex: cortex.to_account_info(),
                    perpetuals: perpetuals.to_account_info(),
                    staking_reward_token_mint: ctx
                        .accounts
                        .staking_reward_token_mint
                        .to_account_info(),
                    lm_token_mint: ctx.accounts.lm_token_mint.to_account_info(),
                    perpetuals_program: ctx.accounts.perpetuals_program.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                };

                let cpi_program = ctx.accounts.perpetuals_program.to_account_info();
                crate::cpi::claim_stakes(CpiContext::new(cpi_program, cpi_accounts))?;

                // Force reloading all accounts that may have been affected by claim
                {
                    ctx.accounts.reward_token_account.reload()?;
                    ctx.accounts.lm_token_account.reload()?;
                    ctx.accounts.staking_reward_token_vault.reload()?;
                    ctx.accounts.staking_lm_reward_token_vault.reload()?;
                    staking.reload()?;
                    user_staking.reload()?;
                    cortex.reload()?;
                    perpetuals.reload()?;
                    ctx.accounts.lm_token_mint.reload()?;
                    ctx.accounts.staking_reward_token_mint.reload()?;
                }
            }

            // Set an overlap time to know when the current round should be treated differently than next round
            //
            // will be used during claim_stakes to calculate appropriate rewards
            user_staking.liquid_stake.overlap_time = perpetuals.get_time()?;
            user_staking.liquid_stake.overlap_amount =
                math::checked_add(user_staking.liquid_stake.overlap_amount, params.amount)?;
        } else {
            user_staking.liquid_stake.stake_time = perpetuals.get_time()?;
        }

        user_staking.liquid_stake.amount =
            math::checked_add(user_staking.liquid_stake.amount, params.amount)?;
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
    //   lm rewards           |  0 |
    //   ---------------------------
    //
    if staking.staking_type == StakingType::LM {
        // Give 1:1 governing power to the Stake owner
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
            params.amount,
            None,
            true,
        )?;

        staking.next_staking_round.total_stake =
            math::checked_add(staking.next_staking_round.total_stake, params.amount)?;
    }

    //
    //           LP Staking
    //   ---------------------------
    //   voting power         |  0 |
    //   real yield rewards   |  0 |
    //   lm rewards           | x1 |
    //   ---------------------------
    //
    if staking.staking_type == StakingType::LP {
        staking.next_staking_round.lm_total_stake =
            math::checked_add(staking.next_staking_round.lm_total_stake, params.amount)?;
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
                    ctx.accounts.user_staking.key().as_ref(),
                    &[ctx.accounts.user_staking.thread_authority_bump],
                ]],
            ))?;
        }
    }

    msg!(
        "Current staking round : {:?}",
        staking.current_staking_round
    );
    msg!("Next staking round : {:?}", staking.next_staking_round);

    Ok(())
}
