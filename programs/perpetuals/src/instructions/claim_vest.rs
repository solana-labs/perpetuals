//! AddVest instruction handler

use crate::error::PerpetualsError;

use {
    crate::state::{
        cortex::Cortex,
        multisig::{AdminInstruction, Multisig},
        perpetuals::Perpetuals,
        vest::Vest,
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
};

#[derive(Accounts)]
#[instruction()]
pub struct ClaimVest<'info> {
    #[account()]
    pub owner: Signer<'info>,

    #[account(
        mut,
        constraint = receiving_account.mint == cortex.lm_token_mint,
        has_one = owner
    )]
    pub receiving_account: Box<Account<'info, TokenAccount>>,

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
        mut,
        realloc = Cortex::LEN + (cortex.vests.len() + 1) * std::mem::size_of::<Vest>(),
        realloc::payer = admin,
        realloc::zero = false,
        seeds = [b"cortex"],
        bump = cortex.cortex_bump
    )]
    pub cortex: Box<Account<'info, Cortex>>,

    #[account(
        mut,
        seeds = [b"perpetuals"],
        bump = perpetuals.perpetuals_bump
    )]
    pub perpetuals: Box<Account<'info, Perpetuals>>,

    #[account(
        mut,
        seeds = [b"vest", beneficiary.key().as_ref()],
        bump
    )]
    pub vest: Box<Account<'info, Vest>>,

    #[account(
        mut,
        seeds = [b"lm_token_mint"],
        bump
    )]
    pub lm_token_mint: Box<Account<'info, Mint>>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

pub fn claim_vest<'info>(ctx: Context<'_, '_, '_, 'info, ClaimVest<'info>>) -> Result<u8> {
    let vest = ctx.accounts.vest.as_mut();

    // validate owner
    require!(
        vest.owner == ctx.accounts.owner.key(),
        PerpetualsError::InvalidVestState
    );

// validate maturation of vest
    require!(

    // record vest data
    let cortex = ctx.accounts.cortex.as_mut();
    if vest.inception_time != 0 {
        // return error if pool is already initialized
        return Err(ProgramError::AccountAlreadyInitialized.into());
    }
    msg!(
        "Record vest: share {}%, beneficiary {}",
        params.share,
        ctx.accounts.beneficiary.key
    );
    vest.owner = ctx.accounts.beneficiary.key();
    vest.share = params.share.clone();
    vest.bump = *ctx.bumps.get("vest").ok_or(ProgramError::InvalidSeeds)?;
    vest.inception_time = ctx.accounts.perpetuals.get_time()?;

    cortex.vests.push(ctx.accounts.vest.key());

    Ok(0)
}
