//! UpdatePoolAum instruction handler

use {
    crate::state::{
        perpetuals::Perpetuals,
        pool::{AumCalcMode, Pool},
    },
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
pub struct UpdatePoolAum<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        seeds = [b"perpetuals"],
        bump = perpetuals.perpetuals_bump
    )]
    pub perpetuals: Box<Account<'info, Perpetuals>>,

    #[account(
        mut,
        seeds = [b"pool",
                 pool.name.as_bytes()],
        bump = pool.bump
    )]
    pub pool: Box<Account<'info, Pool>>,
    // remaining accounts:
    //   pool.tokens.len() custody accounts (read-only, unsigned)
    //   pool.tokens.len() custody oracles (read-only, unsigned)
}

pub fn update_pool_aum(ctx: Context<UpdatePoolAum>) -> Result<u128> {
    let perpetuals: &Account<'_, Perpetuals> = ctx.accounts.perpetuals.as_ref();
    let pool = ctx.accounts.pool.as_mut();

    let curtime: i64 = perpetuals.get_time()?;

    // update pool stats
    msg!("Update pool asset under management");

    msg!("Previous value: {}", pool.aum_usd);

    pool.aum_usd =
        pool.get_assets_under_management_usd(AumCalcMode::EMA, ctx.remaining_accounts, curtime)?;

    msg!("Updated value: {}", pool.aum_usd);

    Ok(pool.aum_usd)
}
