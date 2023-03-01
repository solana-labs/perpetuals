//! AddVest instruction handler

use crate::error::PerpetualsError;

use {
    crate::state::{cortex::Cortex, perpetuals::Perpetuals, vest::Vest},
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
};

#[derive(Accounts)]
#[instruction()]
pub struct ClaimVest<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        constraint = receiving_account.mint == lm_token_mint.key(),
        has_one = owner
    )]
    pub receiving_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    #[account(
        mut,
        realloc = Cortex::LEN + (cortex.vests.len() + 1) * std::mem::size_of::<Vest>(),
        realloc::payer = owner,
        realloc::zero = false,
        seeds = [b"cortex"],
        bump = cortex.bump
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
        seeds = [b"vest", owner.key().as_ref()],
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
        vest.is_claimable(ctx.accounts.lm_token_mint.supply)?,
        PerpetualsError::InvalidVestState
    );

    // TODO
    // 1) Transfer tokens from gov to user

    // remove vest from the list
    let cortex = ctx.accounts.cortex.as_mut();
    let vest_idx = cortex
        .vests
        .iter()
        .position(|x| *x == ctx.accounts.vest.key())
        .ok_or(PerpetualsError::InvalidVestState)?;
    cortex.vests.remove(vest_idx);

    // Note: the vest PDA still lives, we can unalloc (currently works same as Pool, without removal)

    Ok(0)
}
