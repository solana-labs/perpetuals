//! InitStaking instruction handler

use {
    crate::state::{
        cortex::Cortex,
        multisig::{AdminInstruction, Multisig},
        perpetuals::Perpetuals,
        staking::{Staking, StakingRound, StakingType},
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
    solana_program::program_error::ProgramError,
};

#[derive(Accounts)]
pub struct InitStaking<'info> {
    #[account()]
    pub admin: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"multisig"],
        bump = multisig.load()?.bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    #[account(
        init_if_needed,
        payer = payer,
        space = Staking::LEN,
        seeds = [b"staking", staking_staked_token_mint.key().as_ref()],
        bump
    )]
    pub staking: Box<Account<'info, Staking>>,

    #[account(
        mut,
        seeds = [b"lm_token_mint"],
        bump = cortex.lm_token_bump
    )]
    pub lm_token_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        seeds = [b"cortex"],
        bump = cortex.bump,
    )]
    pub cortex: Box<Account<'info, Cortex>>,

    #[account(
        mut,
        seeds = [b"perpetuals"],
        bump = perpetuals.perpetuals_bump
    )]
    pub perpetuals: Box<Account<'info, Perpetuals>>,

    #[account(
        init_if_needed,
        payer = payer,
        token::mint = staking_staked_token_mint,
        token::authority = transfer_authority,
        seeds = [b"staking_staked_token_vault", staking.key().as_ref()],
        bump
    )]
    pub staking_staked_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = payer,
        token::mint = staking_reward_token_mint,
        token::authority = transfer_authority,
        seeds = [b"staking_reward_token_vault", staking.key().as_ref()],
        bump
    )]
    pub staking_reward_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = payer,
        token::mint = lm_token_mint,
        token::authority = transfer_authority,
        seeds = [b"staking_lm_reward_token_vault", staking.key().as_ref()],
        bump
    )]
    pub staking_lm_reward_token_vault: Box<Account<'info, TokenAccount>>,

    #[account()]
    pub staking_reward_token_mint: Box<Account<'info, Mint>>,

    #[account(
        mint::authority = transfer_authority,
    )]
    pub staking_staked_token_mint: Box<Account<'info, Mint>>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitStakingParams {
    pub staking_type: StakingType,
}

pub fn init_staking<'info>(
    ctx: Context<'_, '_, '_, 'info, InitStaking<'info>>,
    params: &InitStakingParams,
) -> Result<u8> {
    // validate signatures
    {
        let mut multisig = ctx.accounts.multisig.load_mut()?;

        let signatures_left = multisig.sign_multisig(
            &ctx.accounts.admin,
            &Multisig::get_account_infos(&ctx)[1..],
            &Multisig::get_instruction_data(AdminInstruction::InitStaking, params)?,
        )?;

        if signatures_left > 0 {
            msg!(
                "Instruction has been signed but more signatures are required: {}",
                signatures_left
            );
            return Ok(signatures_left);
        }
    }

    // Initialize lp staking
    {
        let staking = ctx.accounts.staking.as_mut();

        staking.bump = *ctx.bumps.get("staking").ok_or(ProgramError::InvalidSeeds)?;
        staking.staked_token_vault_bump = *ctx
            .bumps
            .get("staking_staked_token_vault")
            .ok_or(ProgramError::InvalidSeeds)?;
        staking.reward_token_vault_bump = *ctx
            .bumps
            .get("staking_reward_token_vault")
            .ok_or(ProgramError::InvalidSeeds)?;
        staking.lm_reward_token_vault_bump = *ctx
            .bumps
            .get("staking_lm_reward_token_vault")
            .ok_or(ProgramError::InvalidSeeds)?;

        staking.staking_type = params.staking_type;
        staking.staked_token_mint = ctx.accounts.staking_staked_token_mint.key();
        staking.staked_token_decimals = ctx.accounts.staking_staked_token_mint.decimals;
        staking.reward_token_mint = ctx.accounts.staking_reward_token_mint.key();
        staking.reward_token_decimals = ctx.accounts.staking_reward_token_mint.decimals;
        staking.resolved_reward_token_amount = u64::MIN;
        staking.resolved_staked_token_amount = u64::MIN;
        staking.resolved_lm_reward_token_amount = u64::MIN;
        staking.resolved_lm_staked_token_amount = u64::MIN;
        staking.current_staking_round = StakingRound::new(ctx.accounts.perpetuals.get_time()?);
        staking.next_staking_round = StakingRound::new(0);
        staking.resolved_staking_rounds = Vec::new();
    }

    Ok(0)
}
