//! InitStaking instruction handler

use {
    crate::state::staking::Staking, anchor_lang::prelude::*,
    solana_program::program_error::ProgramError,
};

#[derive(Accounts)]
pub struct InitStaking<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        init,
        payer = owner,
        space = Staking::LEN,
        seeds = [b"staking",
                 owner.key().as_ref()],
        bump
    )]
    pub staking: Box<Account<'info, Staking>>,

    system_program: Program<'info, System>,
}

pub fn init_staking(ctx: Context<InitStaking>) -> Result<()> {
    let staking = ctx.accounts.staking.as_mut();

    staking.bump = *ctx.bumps.get("staking").ok_or(ProgramError::InvalidSeeds)?;

    staking.locked_stakes = Vec::new();

    Ok(())
}
