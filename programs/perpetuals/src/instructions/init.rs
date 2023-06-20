//! Init instruction handler

use {
    crate::{
        adapters::SplGovernanceV3Adapter,
        error::PerpetualsError,
        state::{
            cortex::{Cortex, StakingRound},
            multisig::Multisig,
            perpetuals::Perpetuals,
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
        space = Cortex::LEN + std::mem::size_of::<StakingRound>(),
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

    // staked token vault
    #[account(
        init,
        payer = upgrade_authority,
        token::mint = lm_token_mint,
        token::authority = transfer_authority,
        seeds = [b"staking_token_account"],
        bump
    )]
    pub staking_token_account: Box<Account<'info, TokenAccount>>,

    // staking reward token vault
    #[account(
        init,
        payer = upgrade_authority,
        token::mint = staking_reward_token_mint,
        token::authority = transfer_authority,
        seeds = [b"staking_reward_token_account"],
        bump
    )]
    pub staking_reward_token_account: Box<Account<'info, TokenAccount>>,

    // staking lm reward token vault
    #[account(
        init,
        payer = upgrade_authority,
        token::mint = lm_token_mint,
        token::authority = transfer_authority,
        seeds = [b"staking_lm_reward_token_account"],
        bump
    )]
    pub staking_lm_reward_token_account: Box<Account<'info, TokenAccount>>,

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

    #[account()]
    pub staking_reward_token_mint: Box<Account<'info, Mint>>,

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
        cortex.lm_token_bump = *ctx
            .bumps
            .get("lm_token_mint")
            .ok_or(ProgramError::InvalidSeeds)?;
        cortex.governance_token_bump = *ctx
            .bumps
            .get("governance_token_mint")
            .ok_or(ProgramError::InvalidSeeds)?;
        cortex.bump = *ctx.bumps.get("cortex").ok_or(ProgramError::InvalidSeeds)?;
        cortex.staking_token_account_bump = *ctx
            .bumps
            .get("staking_token_account")
            .ok_or(ProgramError::InvalidSeeds)?;
        cortex.staking_reward_token_account_bump = *ctx
            .bumps
            .get("staking_reward_token_account")
            .ok_or(ProgramError::InvalidSeeds)?;
        cortex.staking_lm_reward_token_account_bump = *ctx
            .bumps
            .get("staking_lm_reward_token_account")
            .ok_or(ProgramError::InvalidSeeds)?;
        cortex.inception_epoch = cortex.get_epoch()?;
        cortex.governance_program = ctx.accounts.governance_program.key();
        cortex.governance_realm = ctx.accounts.governance_realm.key();
        cortex.staking_reward_token_mint = ctx.accounts.staking_reward_token_mint.key();
        cortex.resolved_reward_token_amount = u64::MIN;
        cortex.resolved_stake_token_amount = u128::MIN;
        cortex.stake_token_decimals = ctx.accounts.lm_token_mint.decimals;
        cortex.stake_reward_token_decimals = ctx.accounts.staking_reward_token_mint.decimals;
        // initialize the first staking rounds
        cortex.current_staking_round = StakingRound::new(perpetuals.get_time()?);
        cortex.next_staking_round = StakingRound::new(0);
    }

    Ok(())
}
