//! ClosePosition instruction handler

use {
    crate::{
        error::PerpetualsError,
        instructions::SwapParams,
        math,
        state::{
            cortex::Cortex,
            custody::Custody,
            oracle::OraclePrice,
            perpetuals::Perpetuals,
            pool::Pool,
            position::{Position, Side},
        },
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
    num_traits::Zero,
};

#[derive(Accounts)]
pub struct ClosePosition<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        constraint = receiving_account.mint == custody.mint,
        has_one = owner
    )]
    pub receiving_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = lm_token_account.mint == lm_token_mint.key(),
        has_one = owner
    )]
    pub lm_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    #[account(
        seeds = [b"cortex"],
        bump = cortex.bump
    )]
    pub cortex: Box<Account<'info, Cortex>>,

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
        mut,
        has_one = owner,
        seeds = [b"position",
                 owner.key().as_ref(),
                 pool.key().as_ref(),
                 custody.key().as_ref(),
                 &[position.side as u8]],
        bump = position.bump,
        close = owner
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

    /// CHECK: oracle account for the collateral token
    #[account(
        constraint = custody_oracle_account.key() == custody.oracle.oracle_account
    )]
    pub custody_oracle_account: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"custody_token_account",
                 pool.key().as_ref(),
                 custody.mint.as_ref()],
        bump = custody.token_account_bump
    )]
    pub custody_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [b"custody",
                 pool.key().as_ref(),
                 stake_reward_token_custody.mint.as_ref()],
        bump = stake_reward_token_custody.bump,
        constraint = stake_reward_token_custody.mint == stake_reward_token_mint.key(),
    )]
    pub stake_reward_token_custody: Box<Account<'info, Custody>>,

    /// CHECK: oracle account for the stake_reward token
    #[account(
        constraint = stake_reward_token_custody_oracle_account.key() == stake_reward_token_custody.oracle.oracle_account
    )]
    pub stake_reward_token_custody_oracle_account: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"custody_token_account",
                 pool.key().as_ref(),
                 stake_reward_token_custody.mint.as_ref()],
        bump = stake_reward_token_custody.token_account_bump,
    )]
    pub stake_reward_token_custody_token_account: Box<Account<'info, TokenAccount>>,

    // staking reward token vault (receiving fees swapped to `stake_reward_token_mint`)
    #[account(
        mut,
        token::mint = cortex.stake_reward_token_mint,
        seeds = [b"stake_reward_token_account"],
        bump = cortex.stake_reward_token_account_bump
    )]
    pub stake_reward_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [b"lm_token_mint"],
        bump = cortex.lm_token_bump
    )]
    pub lm_token_mint: Box<Account<'info, Mint>>,

    #[account()]
    pub stake_reward_token_mint: Box<Account<'info, Mint>>,

    token_program: Program<'info, Token>,
    perpetuals_program: Program<'info, Perpetuals>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub struct ClosePositionParams {
    pub price: u64,
}

pub fn close_position(ctx: Context<ClosePosition>, params: &ClosePositionParams) -> Result<()> {
    // check permissions
    msg!("Check permissions");
    let perpetuals = ctx.accounts.perpetuals.as_mut();
    let custody = ctx.accounts.custody.as_mut();
    require!(
        perpetuals.permissions.allow_close_position && custody.permissions.allow_close_position,
        PerpetualsError::InstructionNotAllowed
    );

    // validate inputs
    msg!("Validate inputs");
    if params.price == 0 {
        return Err(ProgramError::InvalidArgument.into());
    }
    let position = ctx.accounts.position.as_mut();
    let pool = ctx.accounts.pool.as_mut();

    // compute exit price
    let curtime = perpetuals.get_time()?;

    let token_price = OraclePrice::new_from_oracle(
        custody.oracle.oracle_type,
        &ctx.accounts.custody_oracle_account.to_account_info(),
        custody.oracle.max_price_error,
        custody.oracle.max_price_age_sec,
        curtime,
        false,
    )?;

    let token_ema_price = OraclePrice::new_from_oracle(
        custody.oracle.oracle_type,
        &ctx.accounts.custody_oracle_account.to_account_info(),
        custody.oracle.max_price_error,
        custody.oracle.max_price_age_sec,
        curtime,
        custody.pricing.use_ema,
    )?;

    let exit_price = pool.get_exit_price(&token_price, &token_ema_price, position.side, custody)?;
    msg!("Exit price: {}", exit_price);

    if position.side == Side::Long {
        require_gte!(exit_price, params.price, PerpetualsError::MaxPriceSlippage);
    } else {
        require_gte!(params.price, exit_price, PerpetualsError::MaxPriceSlippage);
    }

    msg!("Settle position");
    let (transfer_amount, fee_amount, profit_usd, loss_usd) = pool.get_close_amount(
        position,
        &token_price,
        &token_ema_price,
        custody,
        curtime,
        false,
    )?;

    let protocol_fee = Pool::get_fee_amount(custody.fees.protocol_share, fee_amount)?;

    msg!("Net profit: {}, loss: {}", profit_usd, loss_usd);
    msg!("Collected fee: {}", fee_amount);
    msg!("Amount out: {}", transfer_amount);

    // unlock pool funds
    custody.unlock_funds(position.locked_amount)?;

    // check pool constraints
    msg!("Check pool constraints");
    require!(
        pool.check_available_amount(transfer_amount, custody)?,
        PerpetualsError::CustodyAmountLimit
    );

    // transfer tokens
    msg!("Transfer tokens");
    perpetuals.transfer_tokens(
        ctx.accounts.custody_token_account.to_account_info(),
        ctx.accounts.receiving_account.to_account_info(),
        ctx.accounts.transfer_authority.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        transfer_amount,
    )?;

    // LM rewards
    let lm_rewards_amount = {
        // compute amount of lm token to mint
        let amount = ctx.accounts.cortex.get_lm_rewards_amount(fee_amount)?;

        // mint lm tokens
        perpetuals.mint_tokens(
            ctx.accounts.lm_token_mint.to_account_info(),
            ctx.accounts.lm_token_account.to_account_info(),
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            amount,
        )?;
        msg!("Amount LM rewards out: {}", amount);
        amount
    };

    // update custody stats
    msg!("Update custody stats");
    custody.collected_fees.close_position_usd = custody
        .collected_fees
        .close_position_usd
        .wrapping_add(token_ema_price.get_asset_amount_usd(fee_amount, custody.decimals)?);

    custody.distributed_rewards.close_position_lm = custody
        .distributed_rewards
        .close_position_lm
        .wrapping_add(lm_rewards_amount);

    custody.volume_stats.close_position_usd = custody
        .volume_stats
        .close_position_usd
        .wrapping_add(position.size_usd);

    let amount_lost = transfer_amount.saturating_sub(position.collateral_amount);
    custody.assets.owned = math::checked_sub(custody.assets.owned, amount_lost)?;
    custody.assets.collateral =
        math::checked_sub(custody.assets.collateral, position.collateral_amount)?;
    custody.assets.protocol_fees = math::checked_add(custody.assets.protocol_fees, protocol_fee)?;

    if position.side == Side::Long {
        custody.trade_stats.oi_long_usd = custody
            .trade_stats
            .oi_long_usd
            .saturating_sub(position.size_usd);
    } else {
        custody.trade_stats.oi_short_usd = custody
            .trade_stats
            .oi_short_usd
            .saturating_sub(position.size_usd);
    }

    custody.trade_stats.profit_usd = custody.trade_stats.profit_usd.wrapping_add(profit_usd);
    custody.trade_stats.loss_usd = custody.trade_stats.loss_usd.wrapping_add(loss_usd);

    custody.remove_position(position, curtime)?;
    custody.update_borrow_rate(curtime)?;

    // if there is no collected fees, skip transfer to staking vault
    if !protocol_fee.is_zero() {
        // if the collected fees are in the right denomination, skip swap
        if custody.mint == ctx.accounts.stake_reward_token_custody.mint {
            msg!("Transfer collected fees to stake vault (no swap)");
            perpetuals.transfer_tokens(
                ctx.accounts.custody_token_account.to_account_info(),
                ctx.accounts.stake_reward_token_account.to_account_info(),
                ctx.accounts.transfer_authority.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                fee_amount,
            )?;
            // Force sync between two account that are the same in that specific case, and that can have race condition at IX end
            // when accounts state is saved (A is modified not B, A is saved, B is saved and overwrite)
            let srt_custody = ctx.accounts.stake_reward_token_custody.as_mut();
            srt_custody.assets.owned = custody.assets.owned;
            srt_custody.exit(&crate::ID)?;
            srt_custody.reload()?;
        } else {
            // swap the collected fee_amount to stable and send to staking rewards
            msg!("Swap collected fees to stake reward mint internally");
            perpetuals.internal_swap(
                ctx.accounts.transfer_authority.to_account_info(),
                ctx.accounts.custody_token_account.to_account_info(),
                ctx.accounts.stake_reward_token_account.to_account_info(),
                ctx.accounts.lm_token_account.to_account_info(),
                ctx.accounts.cortex.to_account_info(),
                perpetuals.to_account_info(),
                ctx.accounts.pool.to_account_info(),
                custody.to_account_info(),
                ctx.accounts.custody_oracle_account.to_account_info(),
                ctx.accounts.custody_token_account.to_account_info(),
                ctx.accounts.stake_reward_token_custody.to_account_info(),
                ctx.accounts
                    .stake_reward_token_custody_oracle_account
                    .to_account_info(),
                ctx.accounts
                    .stake_reward_token_custody_token_account
                    .to_account_info(),
                ctx.accounts.stake_reward_token_custody.to_account_info(),
                ctx.accounts
                    .stake_reward_token_custody_oracle_account
                    .to_account_info(),
                ctx.accounts
                    .stake_reward_token_custody_token_account
                    .to_account_info(),
                ctx.accounts.stake_reward_token_account.to_account_info(),
                ctx.accounts.stake_reward_token_mint.to_account_info(),
                ctx.accounts.lm_token_mint.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.perpetuals_program.to_account_info(),
                SwapParams {
                    amount_in: protocol_fee,
                    min_amount_out: protocol_fee,
                },
            )?;
        }
    }

    Ok(())
}
