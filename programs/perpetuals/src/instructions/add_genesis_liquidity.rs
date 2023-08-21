//! AddGenesisLiquidity instruction handler

use {
    crate::{
        adapters::SplGovernanceV3Adapter,
        error::PerpetualsError,
        instructions::FinalizeLockedStakeParams,
        math,
        state::{
            cortex::Cortex,
            custody::{get_custody_mint_from_account_info, Custody},
            oracle::OraclePrice,
            perpetuals::Perpetuals,
            pool::{AumCalcMode, Pool},
            staking::Staking,
            user_staking::{
                LockedStake, LockedStakingOption, UserStaking, USER_STAKING_THREAD_AUTHORITY_SEED,
            },
        },
    },
    anchor_lang::{prelude::*, InstructionData},
    anchor_spl::token::{Mint, Token, TokenAccount},
    solana_program::{instruction::Instruction, program_error::ProgramError},
    std::str::FromStr,
};

#[derive(Accounts)]
#[instruction(params: AddGenesisLiquidityParams)]
pub struct AddGenesisLiquidity<'info> {
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

    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"user_staking",
                 owner.key().as_ref(), lp_staking.key().as_ref()],
        bump = lp_user_staking.bump
    )]
    pub lp_user_staking: Box<Account<'info, UserStaking>>,

    #[account(
        mut,
        seeds = [b"staking", lp_token_mint.key().as_ref()],
        bump = lp_staking.bump,
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
        token::mint = lp_staking.staked_token_mint,
        token::authority = transfer_authority,
        seeds = [b"staking_staked_token_vault", lp_staking.key().as_ref()],
        bump = lp_staking.staked_token_vault_bump,
    )]
    pub lp_staking_staked_token_vault: Box<Account<'info, TokenAccount>>,

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

    #[account(
        mut,
        seeds = [b"governance_token_mint"],
        bump = cortex.governance_token_bump
    )]
    pub governance_token_mint: Box<Account<'info, Mint>>,

    /// CHECK: checked by spl governance v3 program
    /// A realm represent one project (ADRENA, MANGO etc.) within the governance program
    pub governance_realm: UncheckedAccount<'info>,

    /// CHECK: checked by spl governance v3 program
    pub governance_realm_config: UncheckedAccount<'info>,

    /// CHECK: checked by spl governance v3 program
    /// Token account owned by governance program holding user's locked tokens
    #[account(mut)]
    pub governance_governing_token_holding: UncheckedAccount<'info>,

    /// CHECK: checked by spl governance v3 program
    /// Account owned by governance storing user informations
    #[account(mut)]
    pub governance_governing_token_owner_record: UncheckedAccount<'info>,

    /// CHECK: checked by clockwork thread program
    #[account(mut)]
    pub lp_stake_resolution_thread: UncheckedAccount<'info>,

    /// CHECK: checked by clockwork thread program
    #[account(mut)]
    pub stakes_claim_cron_thread: Box<Account<'info, clockwork_sdk::state::Thread>>,

    /// CHECK: empty PDA, authority for threads
    #[account(
        seeds = [USER_STAKING_THREAD_AUTHORITY_SEED, lp_user_staking.key().as_ref()],
        bump = lp_user_staking.thread_authority_bump
    )]
    pub lp_user_staking_thread_authority: AccountInfo<'info>,

    clockwork_program: Program<'info, clockwork_sdk::ThreadProgram>,
    governance_program: Program<'info, SplGovernanceV3Adapter>,
    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    perpetuals_program: Program<'info, Perpetuals>,
    // remaining accounts:
    //   pool.tokens.len() custody accounts (read-only, unsigned)
    //   pool.tokens.len() custody oracles (read-only, unsigned)
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddGenesisLiquidityParams {
    pub lp_stake_resolution_thread_id: u64,
    pub amount_in: u64,
    pub min_lp_amount_out: u64,
}

pub fn add_genesis_liquidity(
    ctx: Context<AddGenesisLiquidity>,
    params: &AddGenesisLiquidityParams,
) -> Result<()> {
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

    let min_price = if token_price < token_ema_price {
        token_price
    } else {
        token_ema_price
    };

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

    let token_amount_usd = min_price.get_asset_amount_usd(params.amount_in, custody.decimals)?;

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

    {
        msg!("Update custody stats");
        custody.volume_stats.add_liquidity_usd =
            custody.volume_stats.add_liquidity_usd.wrapping_add(
                token_ema_price.get_asset_amount_usd(params.amount_in, custody.decimals)?,
            );

        custody.assets.owned = math::checked_add(custody.assets.owned, params.amount_in)?;

        custody.update_borrow_rate(curtime)?;
    }

    // Check we do not go over Genesis ALP limits (only in prod)
    {
        // Addresses
        //
        let (usdc, eth, btc, wsol) = {
            // For tests
            // assume custodies based on their position in the array because of changing mints
            // enough to run unit tests
            if cfg!(feature = "test") {
                (
                    get_custody_mint_from_account_info(&ctx.remaining_accounts[0]),
                    get_custody_mint_from_account_info(&ctx.remaining_accounts[1]),
                    get_custody_mint_from_account_info(&ctx.remaining_accounts[2]),
                    get_custody_mint_from_account_info(&ctx.remaining_accounts[3]),
                )
            } else {
                // For prod
                (
                    // Wrapped Bitcoin (Sollet)
                    Pubkey::from_str("9n4nbM75f5Ui33ZbPYXn59EwSgE8CGsHtAeTH5YFeJ9E").unwrap(),
                    // Wrapped Ethereum (Sollet)
                    Pubkey::from_str("2FPyTwcZLUg1MDrwsyoP4D6s1tM7hAkHYRjkNb5w6Pxk").unwrap(),
                    Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap(),
                    Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap(),
                )
            }
        };

        // Limits
        //
        let btc_usd_limit = 100_000 * 10u64.pow(6);
        let eth_usd_limit = 100_000 * 10u64.pow(6);
        let wsol_usd_limit = 100_000 * 10u64.pow(9);
        let usdc_usd_limit = 100_000 * 10u64.pow(6);

        if !custody.mint.eq(&btc)
            && !custody.mint.eq(&eth)
            && !custody.mint.eq(&wsol)
            && !custody.mint.eq(&usdc)
        {
            // Not handled custody
            return Err(PerpetualsError::InvalidCustodyState.into());
        }

        if custody.mint.eq(&btc) && custody.volume_stats.add_liquidity_usd > btc_usd_limit {
            return Err(PerpetualsError::GenesisAlpLimitReached.into());
        }
        if custody.mint.eq(&eth) && custody.volume_stats.add_liquidity_usd > eth_usd_limit {
            return Err(PerpetualsError::GenesisAlpLimitReached.into());
        }
        if custody.mint.eq(&wsol) && custody.volume_stats.add_liquidity_usd > wsol_usd_limit {
            return Err(PerpetualsError::GenesisAlpLimitReached.into());
        }
        if custody.mint.eq(&usdc) && custody.volume_stats.add_liquidity_usd > usdc_usd_limit {
            return Err(PerpetualsError::GenesisAlpLimitReached.into());
        }
    }

    let perpetuals = ctx.accounts.perpetuals.as_mut();

    // mint lp tokens to staking vault directly
    perpetuals.mint_tokens(
        ctx.accounts.lp_token_mint.to_account_info(),
        ctx.accounts.lp_staking_staked_token_vault.to_account_info(),
        ctx.accounts.transfer_authority.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        lp_amount,
    )?;

    // update pool stats
    msg!("Update pool stats");
    custody.exit(&crate::ID)?;
    pool.aum_usd =
        pool.get_assets_under_management_usd(AumCalcMode::EMA, ctx.remaining_accounts, curtime)?;

    msg!("pool.aum_usd: {}", pool.aum_usd);

    let lp_user_staking = ctx.accounts.lp_user_staking.as_mut();
    let cortex = ctx.accounts.cortex.as_mut();
    let lp_staking = ctx.accounts.lp_staking.as_mut();

    let staking_option = LockedStakingOption {
        locked_days: 30,
        reward_multiplier: 1,
        lm_reward_multiplier: (Perpetuals::BPS_POWER as f64 * 1.5) as u32,
        vote_multiplier: 1,
    };

    // Add stake to UserStaking account
    let (stake_amount_with_reward_multiplier, stake_amount_with_lm_reward_multiplier) = {
        let stake_amount_with_reward_multiplier = math::checked_as_u64(math::checked_div(
            math::checked_mul(lp_amount, staking_option.reward_multiplier as u64)? as u128,
            Perpetuals::BPS_POWER,
        )?)?;

        let stake_amount_with_lm_reward_multiplier = math::checked_as_u64(math::checked_div(
            math::checked_mul(lp_amount, staking_option.lm_reward_multiplier as u64)? as u128,
            Perpetuals::BPS_POWER,
        )?)?;

        // Add the new locked staking to the list
        lp_user_staking.locked_stakes.push(LockedStake {
            amount: lp_amount,
            stake_time: perpetuals.get_time()?,
            claim_time: 0,

            // Transform days in seconds here
            lock_duration: math::checked_mul(staking_option.locked_days as u64, 3_600 * 24)?,
            reward_multiplier: staking_option.reward_multiplier,
            lm_reward_multiplier: staking_option.lm_reward_multiplier,
            vote_multiplier: staking_option.vote_multiplier,

            amount_with_reward_multiplier: stake_amount_with_reward_multiplier,
            amount_with_lm_reward_multiplier: stake_amount_with_lm_reward_multiplier,

            resolved: false,
            stake_resolution_thread_id: params.lp_stake_resolution_thread_id,
        });

        // Adapt the size of the staking account
        Perpetuals::realloc(
            ctx.accounts.owner.to_account_info(),
            lp_user_staking.clone().to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            lp_user_staking.size(),
            false,
        )?;

        // Create a clockwork thread to auto-resolve the staking when it ends
        {
            clockwork_sdk::cpi::thread_create(
                CpiContext::new_with_signer(
                    ctx.accounts.clockwork_program.to_account_info(),
                    clockwork_sdk::cpi::ThreadCreate {
                        payer: ctx.accounts.owner.to_account_info(),
                        system_program: ctx.accounts.system_program.to_account_info(),
                        thread: ctx.accounts.lp_stake_resolution_thread.to_account_info(),
                        authority: ctx
                            .accounts
                            .lp_user_staking_thread_authority
                            .to_account_info(),
                    },
                    &[&[
                        USER_STAKING_THREAD_AUTHORITY_SEED,
                        lp_user_staking.key().as_ref(),
                        &[lp_user_staking.thread_authority_bump],
                    ]],
                ),
                // Lamports paid to the clockwork worker executing the thread
                math::checked_add(
                    UserStaking::AUTOMATION_EXEC_FEE,
                    // Provide enough for the thread account to be rent exempt
                    Rent::get()?
                        .minimum_balance(ctx.accounts.lp_stake_resolution_thread.data_len()),
                )?,
                params.lp_stake_resolution_thread_id.try_to_vec().unwrap(),
                //
                // Instruction to be executed with the thread
                vec![Instruction {
                    program_id: crate::ID,
                    accounts: crate::cpi::accounts::FinalizeLockedStake {
                        caller: ctx.accounts.lp_stake_resolution_thread.to_account_info(),
                        owner: ctx.accounts.owner.to_account_info(),
                        transfer_authority: ctx.accounts.transfer_authority.to_account_info(),
                        user_staking: lp_user_staking.to_account_info(),
                        staking: lp_staking.to_account_info(),
                        cortex: cortex.to_account_info(),
                        perpetuals: perpetuals.to_account_info(),
                        lm_token_mint: ctx.accounts.lm_token_mint.to_account_info(),
                        governance_token_mint: ctx.accounts.governance_token_mint.to_account_info(),
                        governance_realm: ctx.accounts.governance_realm.to_account_info(),
                        governance_realm_config: ctx
                            .accounts
                            .governance_realm_config
                            .to_account_info(),
                        governance_governing_token_holding: ctx
                            .accounts
                            .governance_governing_token_holding
                            .to_account_info(),
                        governance_governing_token_owner_record: ctx
                            .accounts
                            .governance_governing_token_owner_record
                            .to_account_info(),
                        governance_program: ctx.accounts.governance_program.to_account_info(),
                        perpetuals_program: ctx.accounts.perpetuals_program.to_account_info(),
                        system_program: ctx.accounts.system_program.to_account_info(),
                        token_program: ctx.accounts.token_program.to_account_info(),
                    }
                    .to_account_metas(Some(true)),
                    data: crate::instruction::FinalizeLockedStake {
                        params: FinalizeLockedStakeParams {
                            thread_id: params.lp_stake_resolution_thread_id,
                        },
                    }
                    .data(),
                }
                .into()],
                //
                // Trigger configuration
                clockwork_sdk::state::Trigger::Timestamp {
                    unix_ts: staking_option.calculate_end_of_staking(perpetuals.get_time()?)?,
                },
            )?;
        }

        (
            stake_amount_with_reward_multiplier,
            stake_amount_with_lm_reward_multiplier,
        )
    };

    // Adapt staking account to newly staked tokens
    {
        lp_staking.next_staking_round.total_stake = math::checked_add(
            lp_staking.next_staking_round.total_stake,
            stake_amount_with_reward_multiplier,
        )?;

        lp_staking.next_staking_round.lm_total_stake = math::checked_add(
            lp_staking.next_staking_round.lm_total_stake,
            stake_amount_with_lm_reward_multiplier,
        )?;

        lp_staking.nb_locked_tokens = math::checked_add(lp_staking.nb_locked_tokens, lp_amount)?;
    }

    Ok(())
}
