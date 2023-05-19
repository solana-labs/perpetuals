//! GetLpTokenPrice instruction handler

use {
    crate::{
        math::{checked_as_f64, checked_float_div, to_token_amount},
        state::{
            perpetuals::Perpetuals,
            pool::{AumCalcMode, Pool},
        },
    },
    anchor_lang::prelude::*,
    anchor_spl::token::Mint,
    num_traits::Zero,
};

#[derive(Accounts)]
pub struct GetLpTokenPrice<'info> {
    #[account(
        seeds = [b"perpetuals"],
        bump = perpetuals.perpetuals_bump
    )]
    pub perpetuals: Box<Account<'info, Perpetuals>>,

    #[account(
        seeds = [b"pool",
                 pool.name.as_bytes()],
        bump = pool.bump
    )]
    pub pool: Box<Account<'info, Pool>>,

    #[account(
        seeds = [b"lp_token_mint",
                 pool.key().as_ref()],
        bump = pool.lp_token_bump
    )]
    pub lp_token_mint: Box<Account<'info, Mint>>,
    // remaining accounts:
    //   pool.tokens.len() custody accounts (read-only, unsigned)
    //   pool.tokens.len() custody oracles (read-only, unsigned)
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct GetLpTokenPriceParams {}

pub fn get_lp_token_price(
    ctx: Context<GetLpTokenPrice>,
    _params: &GetLpTokenPriceParams,
) -> Result<u64> {
    let aum_usd = checked_as_f64(ctx.accounts.pool.get_assets_under_management_usd(
        AumCalcMode::EMA,
        ctx.remaining_accounts,
        ctx.accounts.perpetuals.get_time()?,
    )?)?;

    msg!("aum_usd: {}", aum_usd);

    let lp_supply = checked_as_f64(ctx.accounts.lp_token_mint.supply)?;

    msg!("lp_supply: {}", lp_supply);

    if lp_supply.is_zero() {
        return Ok(0);
    }

    let ui_price_usd = checked_float_div(aum_usd, lp_supply)?;

    msg!("ui_price_usd: {}", ui_price_usd);

    let price_usd = to_token_amount(ui_price_usd, Perpetuals::USD_DECIMALS.into())?;

    msg!("price_usd: {}", price_usd);

    Ok(price_usd)
}
