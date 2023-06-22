//! Init instruction handler

use {
    crate::{
        adapters::SplGovernanceV3Adapter,
        error::PerpetualsError,
        state::{
            cortex::Cortex,
            multisig::Multisig,
            perpetuals::Perpetuals,
            staking::{Staking, StakingRound, StakingType},
        },
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

    #[account(
        init,
        payer = upgrade_authority,
        space = Staking::LEN,
        seeds = [b"staking", (StakingType::LM as u64).to_be_bytes().as_ref()],
        bump
    )]
    pub lm_staking: Box<Account<'info, Staking>>,

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

pub fn init(ctx: Context<Init>, params: &InitParams) -> Result<()> {
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
    let perpetuals = {
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
        perpetuals
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

        // LM Staking
        {
            let lm_staking = ctx.accounts.lm_staking.as_mut();

            lm_staking.bump = *ctx
                .bumps
                .get("lm_staking")
                .ok_or(ProgramError::InvalidSeeds)?;
            lm_staking.staked_token_vault_bump = *ctx
                .bumps
                .get("lm_staking_staked_token_vault")
                .ok_or(ProgramError::InvalidSeeds)?;
            lm_staking.reward_token_vault_bump = *ctx
                .bumps
                .get("lm_staking_reward_token_vault")
                .ok_or(ProgramError::InvalidSeeds)?;
            lm_staking.lm_reward_token_vault_bump = *ctx
                .bumps
                .get("lm_staking_lm_reward_token_vault")
                .ok_or(ProgramError::InvalidSeeds)?;

            lm_staking.staking_type = StakingType::LM;
            lm_staking.staked_token_mint = ctx.accounts.lm_token_mint.key();
            lm_staking.staked_token_decimals = ctx.accounts.lm_token_mint.decimals;
            lm_staking.reward_token_decimals = ctx.accounts.lm_staking_reward_token_mint.decimals;
            lm_staking.resolved_reward_token_amount = u64::MIN;
            lm_staking.resolved_staked_token_amount = u128::MIN;
            lm_staking.resolved_lm_reward_token_amount = u64::MIN;
            lm_staking.resolved_lm_staked_token_amount = u128::MIN;
            lm_staking.current_staking_round = StakingRound::new(perpetuals.get_time()?);
            lm_staking.next_staking_round = StakingRound::new(0);
            lm_staking.resolved_staking_rounds = Vec::new();
            lm_staking.reward_token_mint = ctx.accounts.lm_staking_reward_token_mint.key();
        }
    }

    Ok(())
}
