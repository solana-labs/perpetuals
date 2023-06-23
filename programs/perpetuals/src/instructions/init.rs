//! Init instruction handler

use {
    super::InitStakingParams,
    crate::{
        adapters::SplGovernanceV3Adapter,
        error::PerpetualsError,
        state::{cortex::Cortex, multisig::Multisig, perpetuals::Perpetuals, staking::StakingType},
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
    solana_program::program_error::ProgramError,
};

#[derive(Accounts)]
pub struct Init<'info> {
    #[account(mut)]
    pub upgrade_authority: Signer<'info>,

    #[account(
        init,
        payer = upgrade_authority,
        space = Multisig::LEN,
        seeds = [b"multisig"],
        bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

    /// CHECK: empty PDA, will be set as authority for token accounts
    #[account(
        init,
        payer = upgrade_authority,
        space = 0,
        seeds = [b"transfer_authority"],
        bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    /// CHECK: checked by init_staking ix
    #[account(mut)]
    pub lm_staking: UncheckedAccount<'info>,

    #[account(
        init,
        payer = upgrade_authority,
        space = Cortex::LEN,
        seeds = [b"cortex"],
        bump
    )]
    pub cortex: Box<Account<'info, Cortex>>,

    #[account(
        init,
        payer = upgrade_authority,
        mint::authority = transfer_authority,
        mint::freeze_authority = transfer_authority,
        mint::decimals = Cortex::LM_DECIMALS,
        seeds = [b"lm_token_mint"],
        bump
    )]
    pub lm_token_mint: Box<Account<'info, Mint>>,

    // the shadow governance token transparently managed by the program (and only the program)
    #[account(
        init,
        payer = upgrade_authority,
        mint::authority = transfer_authority,
        mint::freeze_authority = transfer_authority,
        mint::decimals = Cortex::GOVERNANCE_DECIMALS,
        seeds = [b"governance_token_mint"],
        bump
    )]
    pub governance_token_mint: Box<Account<'info, Mint>>,

    #[account(
        init,
        payer = upgrade_authority,
        token::mint = lm_token_mint,
        token::authority = transfer_authority,
        seeds = [b"staking_staked_token_vault", lm_staking.key().as_ref()],
        bump
    )]
    pub lm_staking_staked_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        init,
        payer = upgrade_authority,
        token::mint = lm_staking_reward_token_mint,
        token::authority = transfer_authority,
        seeds = [b"staking_reward_token_vault", lm_staking.key().as_ref()],
        bump
    )]
    pub lm_staking_reward_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        init,
        payer = upgrade_authority,
        token::mint = lm_token_mint,
        token::authority = transfer_authority,
        seeds = [b"staking_lm_reward_token_vault", lm_staking.key().as_ref()],
        bump
    )]
    pub lm_staking_lm_reward_token_vault: Box<Account<'info, TokenAccount>>,

    #[account()]
    pub lm_staking_reward_token_mint: Box<Account<'info, Mint>>,

    #[account(
        init,
        payer = upgrade_authority,
        space = Perpetuals::LEN,
        seeds = [b"perpetuals"],
        bump
    )]
    pub perpetuals: Box<Account<'info, Perpetuals>>,

    #[account(
        constraint = perpetuals_program.programdata_address()? == Some(perpetuals_program_data.key())
    )]
    pub perpetuals_program: Program<'info, Perpetuals>,

    #[account(
        constraint = perpetuals_program_data.upgrade_authority_address == Some(upgrade_authority.key())
    )]
    pub perpetuals_program_data: Account<'info, ProgramData>,

    /// CHECK: checked by spl governance v3 program
    /// A realm represent one project (ADRENA, MANGO etc.) within the governance program
    pub governance_realm: UncheckedAccount<'info>,

    pub governance_program: Program<'info, SplGovernanceV3Adapter>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
    // remaining accounts: 1 to Multisig::MAX_SIGNERS admin signers (read-only, unsigned)
}

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone)]
pub struct InitParams {
    pub min_signatures: u8,
    pub allow_swap: bool,
    pub allow_add_liquidity: bool,
    pub allow_remove_liquidity: bool,
    pub allow_open_position: bool,
    pub allow_close_position: bool,
    pub allow_pnl_withdrawal: bool,
    pub allow_collateral_withdrawal: bool,
    pub allow_size_change: bool,
    pub core_contributor_bucket_allocation: u64,
    pub dao_treasury_bucket_allocation: u64,
    pub pol_bucket_allocation: u64,
    pub ecosystem_bucket_allocation: u64,
}

pub fn init<'info>(
    ctx: Context<'_, '_, '_, 'info, Init<'info>>,
    params: &InitParams,
) -> Result<()> {
    // initialize multisig, this will fail if account is already initialized
    {
        let mut multisig = ctx.accounts.multisig.load_init()?;

        multisig.set_signers(ctx.remaining_accounts, params.min_signatures)?;

        // record multisig PDA bump
        multisig.bump = *ctx
            .bumps
            .get("multisig")
            .ok_or(ProgramError::InvalidSeeds)?;
    }

    // record perpetuals
    {
        let perpetuals = ctx.accounts.perpetuals.as_mut();
        perpetuals.permissions.allow_swap = params.allow_swap;
        perpetuals.permissions.allow_add_liquidity = params.allow_add_liquidity;
        perpetuals.permissions.allow_remove_liquidity = params.allow_remove_liquidity;
        perpetuals.permissions.allow_open_position = params.allow_open_position;
        perpetuals.permissions.allow_close_position = params.allow_close_position;
        perpetuals.permissions.allow_pnl_withdrawal = params.allow_pnl_withdrawal;
        perpetuals.permissions.allow_collateral_withdrawal = params.allow_collateral_withdrawal;
        perpetuals.permissions.allow_size_change = params.allow_size_change;
        perpetuals.transfer_authority_bump = *ctx
            .bumps
            .get("transfer_authority")
            .ok_or(ProgramError::InvalidSeeds)?;
        perpetuals.perpetuals_bump = *ctx
            .bumps
            .get("perpetuals")
            .ok_or(ProgramError::InvalidSeeds)?;
        perpetuals.inception_time = perpetuals.get_time()?;

        if !perpetuals.validate() {
            return err!(PerpetualsError::InvalidPerpetualsConfig);
        }
    };

    // record cortex
    {
        let cortex = ctx.accounts.cortex.as_mut();

        // Bumps
        {
            cortex.lm_token_bump = *ctx
                .bumps
                .get("lm_token_mint")
                .ok_or(ProgramError::InvalidSeeds)?;
            cortex.governance_token_bump = *ctx
                .bumps
                .get("governance_token_mint")
                .ok_or(ProgramError::InvalidSeeds)?;
            cortex.bump = *ctx.bumps.get("cortex").ok_or(ProgramError::InvalidSeeds)?;
        }

        // Time
        {
            cortex.inception_epoch = cortex.get_epoch()?;
        }

        // Governance
        {
            cortex.governance_program = ctx.accounts.governance_program.key();
            cortex.governance_realm = ctx.accounts.governance_realm.key();
        }

        // Vesting
        {
            cortex.vests = Vec::new();
        }

        // Lm tokens minting rules
        {
            cortex.core_contributor_bucket_allocation = params.core_contributor_bucket_allocation;
            cortex.core_contributor_bucket_minted_amount = u64::MIN;

            cortex.dao_treasury_bucket_allocation = params.dao_treasury_bucket_allocation;
            cortex.dao_treasury_bucket_minted_amount = u64::MIN;

            cortex.pol_bucket_allocation = params.pol_bucket_allocation;
            cortex.pol_bucket_minted_amount = u64::MIN;

            cortex.ecosystem_bucket_allocation = params.ecosystem_bucket_allocation;
            cortex.ecosystem_bucket_minted_amount = u64::MIN;
        }

        // Force the save of the multisig account
        ctx.accounts.multisig.exit(&crate::ID)?;
        ctx.accounts.cortex.exit(&crate::ID)?;
        ctx.accounts.perpetuals.exit(&crate::ID)?;
        ctx.accounts.transfer_authority.exit(&crate::ID)?;
        ctx.accounts.lm_token_mint.exit(&crate::ID)?;

        // Initialize LM Staking
        {
            for i in 0..params.min_signatures {
                let cpi_accounts = crate::cpi::accounts::InitStaking {
                    admin: ctx.remaining_accounts[i as usize].clone(),
                    payer: ctx.accounts.upgrade_authority.to_account_info(),
                    multisig: ctx.accounts.multisig.to_account_info(),
                    transfer_authority: ctx.accounts.transfer_authority.to_account_info(),
                    staking: ctx.accounts.lm_staking.to_account_info(),
                    lm_token_mint: ctx.accounts.lm_token_mint.to_account_info(),
                    cortex: ctx.accounts.cortex.to_account_info().clone(),
                    perpetuals: ctx.accounts.perpetuals.to_account_info(),
                    staking_staked_token_vault: ctx
                        .accounts
                        .lm_staking_staked_token_vault
                        .to_account_info(),
                    staking_reward_token_vault: ctx
                        .accounts
                        .lm_staking_reward_token_vault
                        .to_account_info(),
                    staking_lm_reward_token_vault: ctx
                        .accounts
                        .lm_staking_lm_reward_token_vault
                        .to_account_info(),
                    staking_reward_token_mint: ctx
                        .accounts
                        .lm_staking_reward_token_mint
                        .to_account_info(),
                    staking_staked_token_mint: ctx.accounts.lm_token_mint.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                    rent: ctx.accounts.rent.to_account_info(),
                };

                let cpi_program = ctx.accounts.perpetuals_program.to_account_info();
                crate::cpi::init_staking(
                    CpiContext::new(cpi_program, cpi_accounts),
                    InitStakingParams {
                        staking_type: StakingType::LM,
                    },
                )?;
            }
        }
    }

    Ok(())
}
