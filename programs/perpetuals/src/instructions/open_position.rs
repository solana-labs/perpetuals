//! OpenPosition instruction handler

use {
    crate::{
        error::PerpetualsError,
        math,
        state::{
            custody::Custody,
            oracle::OraclePrice,
            perpetuals::Perpetuals,
            pool::Pool,
            position::{Position, Side},
        },
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Token, TokenAccount},
    solana_program::program_error::ProgramError,
};

#[derive(Accounts)]
#[instruction(params: OpenPositionParams)]
pub struct OpenPosition<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        constraint = funding_account.mint == collateral_custody.mint,
        has_one = owner
    )]
    pub funding_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

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

    #[account(
        init,
        payer = owner,
        space = Position::LEN,
        seeds = [b"position",
                 owner.key().as_ref(),
                 pool.key().as_ref(),
                 custody.key().as_ref(),
                 &[params.side as u8]],
        bump
    )]
    pub position: Box<Account<'info, Position>>,

    #[account(
        mut,
        seeds = [b"custody",
                 pool.key().as_ref(),
                 custody.mint.as_ref()],
        bump = custody.bump
    )]
    pub custody: Box<Account<'info, Custody>>,

    /// CHECK: oracle account for the position token
    #[account(
        constraint = custody_oracle_account.key() == custody.oracle.oracle_account
    )]
    pub custody_oracle_account: AccountInfo<'info>,

    #[account(
        mut,
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

    #[account(
        mut,
        seeds = [b"custody_token_account",
                 pool.key().as_ref(),
                 collateral_custody.mint.as_ref()],
        bump = collateral_custody.token_account_bump
    )]
    pub collateral_custody_token_account: Box<Account<'info, TokenAccount>>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct OpenPositionParams {
    pub price: u64,
    pub collateral: u64,
    pub size: u64,
    pub side: Side,
}

pub fn open_position(ctx: Context<OpenPosition>, params: &OpenPositionParams) -> Result<()> {
    // check permissions
    msg!("Check permissions");
    let perpetuals = ctx.accounts.perpetuals.as_mut();
    let custody = ctx.accounts.custody.as_mut();
    let collateral_custody = ctx.accounts.collateral_custody.as_mut();
    require!(
        perpetuals.permissions.allow_open_position
            && custody.permissions.allow_open_position
            && !custody.is_stable,
        PerpetualsError::InstructionNotAllowed
    );

    // validate inputs
    msg!("Validate inputs");
    if params.price == 0 || params.collateral == 0 || params.size == 0 || params.side == Side::None
    {
        return Err(ProgramError::InvalidArgument.into());
    }
    if params.side == Side::Short || custody.is_virtual {
        require_keys_neq!(custody.key(), collateral_custody.key());
        require!(
            collateral_custody.is_stable && !collateral_custody.is_virtual,
            PerpetualsError::InvalidCollateralCustody
        );
    } else {
        require_keys_eq!(custody.key(), collateral_custody.key());
    };
    let position = ctx.accounts.position.as_mut();
    let pool = ctx.accounts.pool.as_mut();

    // compute position price
    let curtime = perpetuals.get_time()?;

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

    let max_price = if token_price > token_ema_price {
        token_price
    } else {
        token_ema_price
    };

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

    let position_price =
        pool.get_entry_price(&token_price, &token_ema_price, params.side, custody)?;
    msg!("Entry price: {}", position_price);

    if params.side == Side::Long {
        require_gte!(
            params.price,
            position_price,
            PerpetualsError::MaxPriceSlippage
        );
    } else {
        require_gte!(
            position_price,
            params.price,
            PerpetualsError::MaxPriceSlippage
        );
    }

    // compute position parameters
    let size_usd = max_price.get_asset_amount_usd(params.size, custody.decimals)?;
    let collateral_usd = min_collateral_price
        .get_asset_amount_usd(params.collateral, collateral_custody.decimals)?;

    let locked_amount = if params.side == Side::Short || custody.is_virtual {
        custody.get_locked_amount(
            min_collateral_price.get_token_amount(size_usd, collateral_custody.decimals)?,
        )?
    } else {
        custody.get_locked_amount(params.size)?
    };

    // compute fee
    let fee_amount = pool.get_entry_fee(
        custody.fees.open_position,
        params.size,
        locked_amount,
        collateral_custody,
    )?;
    msg!("Collected fee: {}", fee_amount);

    // compute amount to transfer
    let transfer_amount = math::checked_add(params.collateral, fee_amount)?;
    msg!("Amount in: {}", transfer_amount);

    // init new position
    msg!("Initialize new position");
    position.owner = ctx.accounts.owner.key();
    position.pool = pool.key();
    position.custody = custody.key();
    position.collateral_custody = collateral_custody.key();
    position.open_time = perpetuals.get_time()?;
    position.update_time = 0;
    position.side = params.side;
    position.price = position_price;
    position.size_usd = size_usd;
    position.collateral_usd = collateral_usd;
    position.unrealized_profit_usd = 0;
    position.unrealized_loss_usd = 0;
    position.cumulative_interest_snapshot = collateral_custody.get_cumulative_interest(curtime)?;
    position.locked_amount = locked_amount;
    position.collateral_amount = params.collateral;
    position.bump = *ctx
        .bumps
        .get("position")
        .ok_or(ProgramError::InvalidSeeds)?;

    // check position risk
    msg!("Check position risks");
    require!(
        position.locked_amount > 0,
        PerpetualsError::InsufficientAmountReturned
    );
    require!(
        pool.check_leverage(
            position,
            &token_price,
            &token_ema_price,
            custody,
            &collateral_token_price,
            &collateral_token_ema_price,
            collateral_custody,
            curtime,
            true
        )?,
        PerpetualsError::MaxLeverage
    );

    // lock funds for potential profit payoff
    collateral_custody.lock_funds(position.locked_amount)?;

    // transfer tokens
    msg!("Transfer tokens");
    perpetuals.transfer_tokens_from_user(
        ctx.accounts.funding_account.to_account_info(),
        ctx.accounts
            .collateral_custody_token_account
            .to_account_info(),
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        transfer_amount,
    )?;

    // update custody stats
    msg!("Update custody stats");
    collateral_custody.collected_fees.open_position_usd = collateral_custody
        .collected_fees
        .open_position_usd
        .wrapping_add(
            collateral_token_ema_price
                .get_asset_amount_usd(fee_amount, collateral_custody.decimals)?,
        );

    collateral_custody.assets.collateral =
        math::checked_add(collateral_custody.assets.collateral, params.collateral)?;

    let protocol_fee = Pool::get_fee_amount(custody.fees.protocol_share, fee_amount)?;
    collateral_custody.assets.protocol_fees =
        math::checked_add(collateral_custody.assets.protocol_fees, protocol_fee)?;

    // if custody and collateral_custody accounts are the same, ensure that data is in sync
    if position.side == Side::Long && !custody.is_virtual {
        collateral_custody.volume_stats.open_position_usd = collateral_custody
            .volume_stats
            .open_position_usd
            .wrapping_add(size_usd);

        if params.side == Side::Long {
            collateral_custody.trade_stats.oi_long_usd =
                math::checked_add(collateral_custody.trade_stats.oi_long_usd, size_usd)?;
        } else {
            collateral_custody.trade_stats.oi_short_usd =
                math::checked_add(collateral_custody.trade_stats.oi_short_usd, size_usd)?;
        }

        collateral_custody.add_position(position, &token_ema_price, curtime, None)?;
        collateral_custody.update_borrow_rate(curtime)?;
        *custody = collateral_custody.clone();
    } else {
        custody.volume_stats.open_position_usd = custody
            .volume_stats
            .open_position_usd
            .wrapping_add(size_usd);

        if params.side == Side::Long {
            custody.trade_stats.oi_long_usd =
                math::checked_add(custody.trade_stats.oi_long_usd, size_usd)?;
        } else {
            custody.trade_stats.oi_short_usd =
                math::checked_add(custody.trade_stats.oi_short_usd, size_usd)?;
        }

        custody.add_position(
            position,
            &token_ema_price,
            curtime,
            Some(collateral_custody),
        )?;
        collateral_custody.update_borrow_rate(curtime)?;
    }

    Ok(())
}
