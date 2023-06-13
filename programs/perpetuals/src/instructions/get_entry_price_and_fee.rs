//! GetEntryPriceAndFee instruction handler

use {
    crate::state::{
        custody::Custody,
        oracle::OraclePrice,
        perpetuals::{NewPositionPricesAndFee, Perpetuals},
        pool::Pool,
        position::{Position, Side},
    },
    anchor_lang::prelude::*,
    solana_program::program_error::ProgramError,
};

#[derive(Accounts)]
pub struct GetEntryPriceAndFee<'info> {
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
        seeds = [b"custody",
                 pool.key().as_ref(),
                 collateral_custody.mint.as_ref()],
        bump = collateral_custody.bump
    )]
    pub collateral_custody: Box<Account<'info, Custody>>,

    /// CHECK: oracle account for the collateral token
    #[account(
        constraint = collateral_custody_oracle_account.key() == collateral_custody.oracle.oracle_account
    )]
    pub collateral_custody_oracle_account: AccountInfo<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct GetEntryPriceAndFeeParams {
    collateral: u64,
    size: u64,
    side: Side,
}

pub fn get_entry_price_and_fee(
    ctx: Context<GetEntryPriceAndFee>,
    params: &GetEntryPriceAndFeeParams,
) -> Result<NewPositionPricesAndFee> {
    // validate inputs
    if params.collateral == 0 || params.size == 0 || params.side == Side::None {
        return Err(ProgramError::InvalidArgument.into());
    }
    let pool = &ctx.accounts.pool;
    let custody = &ctx.accounts.custody;
    let collateral_custody = &ctx.accounts.collateral_custody;

    // compute position price
    let curtime = ctx.accounts.perpetuals.get_time()?;

    let token_price = OraclePrice::new_from_oracle(
        &ctx.accounts.custody_oracle_account.to_account_info(),
        &custody.oracle,
        curtime,
        false,
    )?;

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

    let min_collateral_price = collateral_token_price
        .get_min_price(&collateral_token_ema_price, collateral_custody.is_stable)?;

    let entry_price = pool.get_entry_price(&token_price, &token_ema_price, params.side, custody)?;

    let position_oracle_price = OraclePrice {
        price: entry_price,
        exponent: -(Perpetuals::PRICE_DECIMALS as i32),
    };
    let size_usd = position_oracle_price.get_asset_amount_usd(params.size, custody.decimals)?;
    let collateral_usd = min_collateral_price
        .get_asset_amount_usd(params.collateral, collateral_custody.decimals)?;

    let locked_amount = if params.side == Side::Short || custody.is_virtual {
        custody.get_locked_amount(
            min_collateral_price.get_token_amount(size_usd, collateral_custody.decimals)?,
        )?
    } else {
        custody.get_locked_amount(params.size)?
    };

    let position = Position {
        side: params.side,
        price: entry_price,
        size_usd,
        collateral_usd,
        cumulative_interest_snapshot: collateral_custody.get_cumulative_interest(curtime)?,
        ..Position::default()
    };

    let liquidation_price = pool.get_liquidation_price(
        &position,
        &token_ema_price,
        custody,
        collateral_custody,
        curtime,
    )?;

    let fee = pool.get_entry_fee(
        custody.fees.open_position,
        params.size,
        locked_amount,
        collateral_custody,
    )?;

    Ok(NewPositionPricesAndFee {
        entry_price,
        liquidation_price,
        fee,
    })
}
