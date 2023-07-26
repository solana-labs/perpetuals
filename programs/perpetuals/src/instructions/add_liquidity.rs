//! AddLiquidity instruction handler

use {
    crate::{
        error::PerpetualsError,
        instructions::{BucketName, MintLmTokensFromBucketParams},
        math,
        state::{
            cortex::Cortex,
            custody::Custody,
            oracle::OraclePrice,
            perpetuals::Perpetuals,
            pool::{AumCalcMode, Pool},
            staking::Staking,
        },
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
    solana_program::program_error::ProgramError,
};

#[derive(Accounts)]
#[instruction(params: AddLiquidityParams)]
pub struct AddLiquidity<'info> {
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
        constraint = lp_token_account.mint == lp_token_mint.key(),
        has_one = owner
    )]
    pub lp_token_account: Box<Account<'info, TokenAccount>>,

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
        mut,
        seeds = [b"staking", lm_token_mint.key().as_ref()],
        bump = lm_staking.bump,
        constraint = lm_staking.reward_token_mint.key() == staking_reward_token_mint.key()
    )]
    pub lm_staking: Box<Account<'info, Staking>>,

    #[account(
        mut,
        seeds = [b"staking", lp_token_mint.key().as_ref()],
        bump = lp_staking.bump,
        constraint = lp_staking.reward_token_mint.key() == staking_reward_token_mint.key()
    )]
    pub lp_staking: Box<Account<'info, Staking>>,

    #[account(
        mut,
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
        seeds = [b"custody",
                 pool.key().as_ref(),
                 staking_reward_token_custody.mint.as_ref()],
        bump = staking_reward_token_custody.bump,
        constraint = staking_reward_token_custody.mint == staking_reward_token_mint.key(),
    )]
    pub staking_reward_token_custody: Box<Account<'info, Custody>>,

    /// CHECK:
    #[account(
        constraint = staking_reward_token_custody_oracle_account.key() == staking_reward_token_custody.oracle.oracle_account
    )]
    pub staking_reward_token_custody_oracle_account: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"custody_token_account",
                 pool.key().as_ref(),
                 staking_reward_token_custody.mint.as_ref()],
        bump = staking_reward_token_custody.token_account_bump,
    )]
    pub staking_reward_token_custody_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [b"custody",
                 pool.key().as_ref(),
                 custody.mint.as_ref()],
        bump = custody.bump
    )]
    pub custody: Box<Account<'info, Custody>>,

    /// CHECK: oracle account for the receiving token
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

    // Note: will be credited with its share of fees swapped from native denomination to `lm_staking_reward_token_mint`
    #[account(
        mut,
        token::mint = lm_staking.reward_token_mint,
        seeds = [b"staking_reward_token_vault", lm_staking.key().as_ref()],
        bump = lm_staking.reward_token_vault_bump
    )]
    pub lm_staking_reward_token_vault: Box<Account<'info, TokenAccount>>,

    // Note: will be credited with its share of fees swapped from native denomination to `lp_staking_reward_token_mint`
    #[account(
        mut,
        token::mint = lp_staking.reward_token_mint,
        seeds = [b"staking_reward_token_vault", lp_staking.key().as_ref()],
        bump = lp_staking.reward_token_vault_bump
    )]
    pub lp_staking_reward_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [b"lm_token_mint"],
        bump = cortex.lm_token_bump
    )]
    pub lm_token_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        seeds = [b"lp_token_mint",
                 pool.key().as_ref()],
        bump = pool.lp_token_bump
    )]
    pub lp_token_mint: Box<Account<'info, Mint>>,

    #[account()]
    pub staking_reward_token_mint: Box<Account<'info, Mint>>,

    token_program: Program<'info, Token>,
    perpetuals_program: Program<'info, Perpetuals>,
    // remaining accounts:
    //   pool.tokens.len() custody accounts (read-only, unsigned)
    //   pool.tokens.len() custody oracles (read-only, unsigned)
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddLiquidityParams {
    pub amount_in: u64,
    pub min_lp_amount_out: u64,
}

pub fn add_liquidity(ctx: Context<AddLiquidity>, params: &AddLiquidityParams) -> Result<()> {
    // check permissions
    msg!("Check permissions");
    let perpetuals = ctx.accounts.perpetuals.as_mut();
    let custody = ctx.accounts.custody.as_mut();
    require!(
        perpetuals.permissions.allow_add_liquidity
            && custody.permissions.allow_add_liquidity
            && !custody.is_virtual,
        PerpetualsError::InstructionNotAllowed
    );

    // validate inputs
    msg!("Validate inputs");
    if params.amount_in == 0 {
        return Err(ProgramError::InvalidArgument.into());
    }

    msg!("amount_in: {}", params.amount_in);

    let pool = ctx.accounts.pool.as_mut();
    let token_id = pool.get_token_id(&custody.key())?;

    // calculate fee
    let curtime = perpetuals.get_time()?;

    // Refresh pool.aum_usm to adapt to token price change
    pool.aum_usd =
        pool.get_assets_under_management_usd(AumCalcMode::EMA, ctx.remaining_accounts, curtime)?;

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

    let min_price = if token_price < token_ema_price {
        token_price
    } else {
        token_ema_price
    };

    let fee_amount =
        pool.get_add_liquidity_fee(token_id, params.amount_in, custody, &token_ema_price)?;
    msg!("Collected fee: {}", fee_amount);

    // check pool constraints
    msg!("Check pool constraints");
    let protocol_fee = Pool::get_fee_amount(custody.fees.protocol_share, fee_amount)?;
    let deposit_amount = math::checked_sub(params.amount_in, protocol_fee)?;
    require!(
        pool.check_token_ratio(token_id, deposit_amount, 0, custody, &token_ema_price)?,
        PerpetualsError::TokenRatioOutOfRange
    );

    // transfer tokens
    msg!("Transfer tokens");
    perpetuals.transfer_tokens_from_user(
        ctx.accounts.funding_account.to_account_info(),
        ctx.accounts.custody_token_account.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        params.amount_in,
    )?;

    // compute assets under management
    msg!("Compute assets under management");
    let pool_amount_usd =
        pool.get_assets_under_management_usd(AumCalcMode::Max, ctx.remaining_accounts, curtime)?;

    // compute amount of lp tokens to mint
    let no_fee_amount = math::checked_sub(params.amount_in, fee_amount)?;
    require_gte!(
        no_fee_amount,
        1u64,
        PerpetualsError::InsufficientAmountReturned
    );

    let token_amount_usd = min_price.get_asset_amount_usd(no_fee_amount, custody.decimals)?;

    let lp_amount = if pool_amount_usd == 0 {
        token_amount_usd
    } else {
        math::checked_as_u64(math::checked_div(
            math::checked_mul(
                token_amount_usd as u128,
                ctx.accounts.lp_token_mint.supply as u128,
            )?,
            pool_amount_usd,
        )?)?
    };
    msg!("LP tokens to mint: {}", lp_amount);

    require!(
        lp_amount >= params.min_lp_amount_out,
        PerpetualsError::MaxPriceSlippage
    );

    // mint lp tokens
    perpetuals.mint_tokens(
        ctx.accounts.lp_token_mint.to_account_info(),
        ctx.accounts.lp_token_account.to_account_info(),
        ctx.accounts.transfer_authority.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        lp_amount,
    )?;

    // LM rewards
    let lm_rewards_amount = {
        // compute amount of lm token to mint
        let amount = ctx.accounts.cortex.get_lm_rewards_amount(fee_amount)?;

        if amount > 0 {
            let cpi_accounts = crate::cpi::accounts::MintLmTokensFromBucket {
                admin: ctx.accounts.transfer_authority.to_account_info(),
                receiving_account: ctx.accounts.lm_token_account.to_account_info(),
                transfer_authority: ctx.accounts.transfer_authority.to_account_info(),
                cortex: ctx.accounts.cortex.to_account_info(),
                perpetuals: perpetuals.to_account_info(),
                lm_token_mint: ctx.accounts.lm_token_mint.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            };

            let cpi_program = ctx.accounts.perpetuals_program.to_account_info();
            crate::cpi::mint_lm_tokens_from_bucket(
                CpiContext::new_with_signer(
                    cpi_program,
                    cpi_accounts,
                    &[&[b"transfer_authority", &[perpetuals.transfer_authority_bump]]],
                ),
                MintLmTokensFromBucketParams {
                    bucket_name: BucketName::Ecosystem,
                    amount,
                    reason: String::from("Liquidity mining rewards"),
                },
            )?;

            {
                ctx.accounts.lm_token_account.reload()?;
                ctx.accounts.cortex.reload()?;
                perpetuals.reload()?;
                ctx.accounts.lm_token_mint.reload()?;
            }
        }

        msg!("Amount LM rewards out: {}", amount);
        amount
    };

    //
    // Calculate fee distribution between (Staked LM, Locked Staked LP, Organic LP)
    //
    let fee_distribution = ctx.accounts.cortex.calculate_fee_distribution(
        math::checked_sub(fee_amount, protocol_fee)?,
        ctx.accounts.lp_token_mint.as_ref(),
        ctx.accounts.lp_staking.as_ref(),
    )?;

    {
        msg!("Update custody stats");
        custody.collected_fees.add_liquidity_usd = custody
            .collected_fees
            .add_liquidity_usd
            .wrapping_add(token_ema_price.get_asset_amount_usd(fee_amount, custody.decimals)?);

        custody.distributed_rewards.add_liquidity_lm = custody
            .distributed_rewards
            .add_liquidity_lm
            .wrapping_add(lm_rewards_amount);

        custody.volume_stats.add_liquidity_usd =
            custody.volume_stats.add_liquidity_usd.wrapping_add(
                token_ema_price.get_asset_amount_usd(params.amount_in, custody.decimals)?,
            );

        custody.assets.protocol_fees =
            math::checked_add(custody.assets.protocol_fees, protocol_fee)?;

        custody.assets.owned = math::checked_add(custody.assets.owned, deposit_amount)?;

        if custody.mint == ctx.accounts.staking_reward_token_custody.mint {
            custody.assets.owned = math::checked_sub(
                custody.assets.owned,
                math::checked_add(
                    fee_distribution.lm_stakers_fee,
                    fee_distribution.locked_lp_stakers_fee,
                )?,
            )?;
        }

        custody.update_borrow_rate(curtime)?;
    }

    // update pool stats
    msg!("Update pool stats");
    custody.exit(&crate::ID)?;
    pool.aum_usd =
        pool.get_assets_under_management_usd(AumCalcMode::EMA, ctx.remaining_accounts, curtime)?;

    msg!("pool.aum_usd: {}", pool.aum_usd);

    //
    // Distribute fees
    //

    let swap_required = custody.mint != ctx.accounts.staking_reward_token_custody.mint;

    // Force save
    {
        perpetuals.exit(&crate::ID)?;
        pool.exit(&crate::ID)?;
        custody.exit(&crate::ID)?;
    }

    ctx.accounts.perpetuals.distribute_fees(
        swap_required,
        fee_distribution,
        ctx.accounts.transfer_authority.to_account_info(),
        ctx.accounts.custody_token_account.as_mut(),
        ctx.accounts.lm_token_account.as_mut(),
        ctx.accounts.cortex.as_mut(),
        ctx.accounts.perpetuals.clone().as_mut(),
        ctx.accounts.pool.as_mut(),
        ctx.accounts.custody.as_mut(),
        ctx.accounts.custody_oracle_account.to_account_info(),
        ctx.accounts.staking_reward_token_custody.as_mut(),
        ctx.accounts
            .staking_reward_token_custody_oracle_account
            .to_account_info(),
        ctx.accounts
            .staking_reward_token_custody_token_account
            .as_mut(),
        ctx.accounts.lm_staking_reward_token_vault.as_mut(),
        ctx.accounts.lp_staking_reward_token_vault.as_mut(),
        ctx.accounts.staking_reward_token_mint.as_mut(),
        ctx.accounts.lm_staking.as_mut(),
        ctx.accounts.lp_staking.as_mut(),
        ctx.accounts.lm_token_mint.as_mut(),
        ctx.accounts.lp_token_mint.as_mut(),
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.perpetuals_program.to_account_info(),
    )?;

    Ok(())
}
