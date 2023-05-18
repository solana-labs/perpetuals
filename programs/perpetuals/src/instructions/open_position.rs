//! OpenPosition instruction handler

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
    solana_program::program_error::ProgramError,
};

#[derive(Accounts)]
#[instruction(params: OpenPositionParams)]
pub struct OpenPosition<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        constraint = funding_account.mint == custody.mint,
        has_one = owner
    )]
    pub funding_account: Box<Account<'info, TokenAccount>>,

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
        bump = cortex.bump,
        has_one = stake_reward_token_mint
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

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    perpetuals_program: Program<'info, Perpetuals>,
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
    let position = ctx.accounts.position.as_mut();
    let pool = ctx.accounts.pool.as_mut();

    // compute position price
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

    let min_price = if token_price < token_ema_price {
        token_price
    } else {
        token_ema_price
    };

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

    // compute fee
    let fee_amount = pool.get_entry_fee(params.size, custody)?;
    msg!("Collected fee: {}", fee_amount);

    // compute amount to transfer
    let transfer_amount = math::checked_add(params.collateral, fee_amount)?;
    msg!("Amount in: {}", transfer_amount);

    // init new position
    msg!("Initialize new position");
    let size_usd = min_price.get_asset_amount_usd(params.size, custody.decimals)?;
    let collateral_usd = min_price.get_asset_amount_usd(params.collateral, custody.decimals)?;

    position.owner = ctx.accounts.owner.key();
    position.pool = pool.key();
    position.custody = custody.key();
    position.open_time = perpetuals.get_time()?;
    position.update_time = 0;
    position.side = params.side;
    position.price = position_price;
    position.size_usd = size_usd;
    position.collateral_usd = collateral_usd;
    position.unrealized_profit_usd = 0;
    position.unrealized_loss_usd = 0;
    position.cumulative_interest_snapshot = custody.get_cumulative_interest(curtime)?;
    position.locked_amount = math::checked_as_u64(math::checked_div(
        math::checked_mul(params.size as u128, custody.pricing.max_payoff_mult as u128)?,
        Perpetuals::BPS_POWER,
    )?)?;

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
        pool.check_leverage(position, &token_ema_price, custody, curtime, true)?,
        PerpetualsError::MaxLeverage
    );

    // lock funds for potential profit payoff
    custody.lock_funds(position.locked_amount)?;

    // transfer tokens
    msg!("Transfer tokens");
    perpetuals.transfer_tokens_from_user(
        ctx.accounts.funding_account.to_account_info(),
        ctx.accounts.custody_token_account.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        transfer_amount,
    )?;

    // compute amount of lm token to mint
    let lm_rewards_amount = ctx.accounts.cortex.get_lm_rewards_amount(fee_amount)?;

    // mint lm tokens
    perpetuals.mint_tokens(
        ctx.accounts.lm_token_mint.to_account_info(),
        ctx.accounts.lm_token_account.to_account_info(),
        ctx.accounts.transfer_authority.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        lm_rewards_amount,
    )?;
    msg!("Amount LM rewards out: {}", lm_rewards_amount);

    // update custody stats
    msg!("Update custody stats");
    custody.collected_fees.open_position_usd = custody
        .collected_fees
        .open_position_usd
        .wrapping_add(token_ema_price.get_asset_amount_usd(fee_amount, custody.decimals)?);

    custody.distributed_rewards.open_position_lm = custody
        .distributed_rewards
        .open_position_lm
        .wrapping_add(lm_rewards_amount);

    custody.distributed_rewards.open_position_lm = custody
        .distributed_rewards
        .open_position_lm
        .wrapping_add(lm_rewards_amount);

    custody.volume_stats.open_position_usd = custody
        .volume_stats
        .open_position_usd
        .wrapping_add(size_usd);

    custody.assets.collateral = math::checked_add(custody.assets.collateral, params.collateral)?;

    let protocol_fee = Pool::get_fee_amount(custody.fees.protocol_share, fee_amount)?;
    custody.assets.protocol_fees = math::checked_add(custody.assets.protocol_fees, protocol_fee)?;

    if params.side == Side::Long {
        custody.trade_stats.oi_long_usd =
            math::checked_add(custody.trade_stats.oi_long_usd, size_usd)?;
    } else {
        custody.trade_stats.oi_short_usd =
            math::checked_add(custody.trade_stats.oi_short_usd, size_usd)?;
    }

    custody.add_position(position, &token_ema_price, curtime)?;
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
