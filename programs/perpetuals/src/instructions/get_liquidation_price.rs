//! GetLiquidationPrice instruction handler

use {
    crate::{
        math,
        state::{
            custody::Custody, oracle::OraclePrice, perpetuals::Perpetuals, pool::Pool,
            position::Position,
        },
    },
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
pub struct GetLiquidationPrice<'info> {
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
        seeds = [b"position",
                 position.owner.as_ref(),
                 pool.key().as_ref(),
                 custody.key().as_ref(),
                 &[position.side as u8]],
        bump = position.bump
    )]
    pub position: Box<Account<'info, Position>>,

    #[account(
        seeds = [b"custody",
                 pool.key().as_ref(),
                 custody.mint.as_ref()],
        bump = custody.bump
    )]
    pub custody: Box<Account<'info, Custody>>,

    /// CHECK: oracle account for the collateral token
    #[account(
        constraint = custody_oracle_account.key() == custody.oracle.oracle_account
    )]
    pub custody_oracle_account: AccountInfo<'info>,

    #[account(
        constraint = position.collateral_custody == collateral_custody.key()
    )]
    pub collateral_custody: Box<Account<'info, Custody>>,

    /// CHECK: oracle account for the collateral token
    #[account(
        constraint = collateral_custody_oracle_account.key() == collateral_custody.oracle.oracle_account
    )]
    pub collateral_custody_oracle_account: AccountInfo<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct GetLiquidationPriceParams {
    add_collateral: u64,
    remove_collateral: u64,
}

pub fn get_liquidation_price(
    ctx: Context<GetLiquidationPrice>,
    params: &GetLiquidationPriceParams,
) -> Result<u64> {
    let custody = &ctx.accounts.custody;
    let collateral_custody = &ctx.accounts.collateral_custody;
    let curtime = ctx.accounts.perpetuals.get_time()?;

    let token_ema_price = OraclePrice::new_from_oracle(
        &ctx.accounts.custody_oracle_account.to_account_info(),
        &custody.oracle,
        curtime,
        custody.pricing.use_ema,
    )?;

    let collateral_token_price = OraclePrice::new_from_oracle(
        &ctx.accounts
            .collateral_custody_oracle_account
            .to_account_info(),
        &collateral_custody.oracle,
        curtime,
        false,
    )?;

    let collateral_token_ema_price = OraclePrice::new_from_oracle(
        &ctx.accounts
            .collateral_custody_oracle_account
            .to_account_info(),
        &collateral_custody.oracle,
        curtime,
        collateral_custody.pricing.use_ema,
    )?;

    let min_collateral_price = if collateral_token_price < collateral_token_ema_price {
        collateral_token_price
    } else {
        collateral_token_ema_price
    };

    let mut position = ctx.accounts.position.clone();
    position.update_time = ctx.accounts.perpetuals.get_time()?;

    if params.add_collateral > 0 {
        let collateral_usd = min_collateral_price
            .get_asset_amount_usd(params.add_collateral, collateral_custody.decimals)?;
        position.collateral_usd = math::checked_add(position.collateral_usd, collateral_usd)?;
        position.collateral_amount =
            math::checked_add(position.collateral_amount, params.add_collateral)?;
    }
    if params.remove_collateral > 0 {
        let collateral_usd = min_collateral_price
            .get_asset_amount_usd(params.remove_collateral, collateral_custody.decimals)?;
        if collateral_usd >= position.collateral_usd
            || params.remove_collateral >= position.collateral_amount
        {
            return Err(ProgramError::InsufficientFunds.into());
        }
        position.collateral_usd = math::checked_sub(position.collateral_usd, collateral_usd)?;
        position.collateral_amount =
            math::checked_sub(position.collateral_amount, params.remove_collateral)?;
    }

    ctx.accounts.pool.get_liquidation_price(
        &position,
        &token_ema_price,
        custody,
        collateral_custody,
        curtime,
    )
}
