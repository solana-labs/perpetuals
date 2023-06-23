//! AddPool instruction handler

use {
    crate::{
        error::PerpetualsError,
        state::{
            cortex::Cortex,
            multisig::{AdminInstruction, Multisig},
            perpetuals::Perpetuals,
            pool::Pool,
            staking::{Staking, StakingRound, StakingType},
        },
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
};

#[derive(Accounts)]
#[instruction(params: AddPoolParams)]
pub struct AddPool<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"multisig"],
        bump = multisig.load()?.bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    #[account(
        init_if_needed,
        payer = admin,
        space = Staking::LEN,
        seeds = [b"staking", lp_token_mint.key().as_ref()],
        bump
    )]
    pub lp_staking: Box<Account<'info, Staking>>,

    #[account(
        mut,
        seeds = [b"lm_token_mint"],
        bump = cortex.lm_token_bump
    )]
    pub lm_token_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        seeds = [b"cortex"],
        bump = cortex.bump,
    )]
    pub cortex: Box<Account<'info, Cortex>>,

    #[account(
        mut,
        realloc = Perpetuals::LEN + (perpetuals.pools.len() + 1) * std::mem::size_of::<Pubkey>(),
        realloc::payer = admin,
        realloc::zero = false,
        seeds = [b"perpetuals"],
        bump = perpetuals.perpetuals_bump
    )]
    pub perpetuals: Box<Account<'info, Perpetuals>>,

    // instruction can be called multiple times due to multisig use, hence init_if_needed
    // instead of init. On the first call account is zero initialized and filled out when
    // all signatures are collected. When account is in zeroed state it can't be used in other
    // instructions because seeds are computed with the pool name. Uniqueness is enforced
    // manually in the instruction handler.
    #[account(
        init_if_needed,
        payer = admin,
        space = Pool::LEN,
        seeds = [b"pool",
                 params.name.as_bytes()],
        bump
    )]
    pub pool: Box<Account<'info, Pool>>,

    #[account(
        init_if_needed,
        payer = admin,
        mint::authority = transfer_authority,
        mint::freeze_authority = transfer_authority,
        mint::decimals = Perpetuals::LP_DECIMALS,
        seeds = [b"lp_token_mint",
                 pool.key().as_ref()],
        bump
    )]
    pub lp_token_mint: Box<Account<'info, Mint>>,

    #[account(
        init_if_needed,
        payer = admin,
        token::mint = lp_token_mint,
        token::authority = transfer_authority,
        seeds = [b"staking_staked_token_vault", lp_staking.key().as_ref()],
        bump
    )]
    pub lp_staking_staked_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = admin,
        token::mint = lp_staking_reward_token_mint,
        token::authority = transfer_authority,
        seeds = [b"staking_reward_token_vault", lp_staking.key().as_ref()],
        bump
    )]
    pub lp_staking_reward_token_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = admin,
        token::mint = lm_token_mint,
        token::authority = transfer_authority,
        seeds = [b"staking_lm_reward_token_vault", lp_staking.key().as_ref()],
        bump
    )]
    pub lp_staking_lm_reward_token_vault: Box<Account<'info, TokenAccount>>,

    #[account()]
    pub lp_staking_reward_token_mint: Box<Account<'info, Mint>>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddPoolParams {
    pub name: String,
}

pub fn add_pool<'info>(
    ctx: Context<'_, '_, '_, 'info, AddPool<'info>>,
    params: &AddPoolParams,
) -> Result<u8> {
    // validate inputs
    {
        if params.name.is_empty() || params.name.len() > 64 {
            return Err(ProgramError::InvalidArgument.into());
        }
    }

    // validate signatures
    {
        let mut multisig = ctx.accounts.multisig.load_mut()?;

        let signatures_left = multisig.sign_multisig(
            &ctx.accounts.admin,
            &Multisig::get_account_infos(&ctx)[1..],
            &Multisig::get_instruction_data(AdminInstruction::AddPool, params)?,
        )?;

        if signatures_left > 0 {
            msg!(
                "Instruction has been signed but more signatures are required: {}",
                signatures_left
            );
            return Ok(signatures_left);
        }
    }

    // record pool data
    {
        let perpetuals = ctx.accounts.perpetuals.as_mut();
        let pool = ctx.accounts.pool.as_mut();

        if pool.inception_time != 0 {
            // return error if pool is already initialized
            return Err(ProgramError::AccountAlreadyInitialized.into());
        }

        msg!("Record pool: {}", params.name);

        pool.inception_time = perpetuals.get_time()?;
        pool.name = params.name.clone();
        pool.bump = *ctx.bumps.get("pool").ok_or(ProgramError::InvalidSeeds)?;
        pool.lp_token_bump = *ctx
            .bumps
            .get("lp_token_mint")
            .ok_or(ProgramError::InvalidSeeds)?;

        if !pool.validate() {
            return err!(PerpetualsError::InvalidPoolConfig);
        }

        perpetuals.pools.push(ctx.accounts.pool.key());
    }

    // Initialize staking
    {
        let lp_staking = ctx.accounts.lp_staking.as_mut();

        lp_staking.bump = *ctx
            .bumps
            .get("lp_staking")
            .ok_or(ProgramError::InvalidSeeds)?;
        lp_staking.staked_token_vault_bump = *ctx
            .bumps
            .get("lp_staking_staked_token_vault")
            .ok_or(ProgramError::InvalidSeeds)?;
        lp_staking.reward_token_vault_bump = *ctx
            .bumps
            .get("lp_staking_reward_token_vault")
            .ok_or(ProgramError::InvalidSeeds)?;
        lp_staking.lm_reward_token_vault_bump = *ctx
            .bumps
            .get("lp_staking_lm_reward_token_vault")
            .ok_or(ProgramError::InvalidSeeds)?;

        lp_staking.staking_type = StakingType::LP;
        lp_staking.staked_token_mint = ctx.accounts.lp_token_mint.key();
        lp_staking.staked_token_decimals = ctx.accounts.lp_token_mint.decimals;
        lp_staking.reward_token_mint = ctx.accounts.lp_staking_reward_token_mint.key();
        lp_staking.reward_token_decimals = ctx.accounts.lp_staking_reward_token_mint.decimals;
        lp_staking.resolved_reward_token_amount = u64::MIN;
        lp_staking.resolved_staked_token_amount = u128::MIN;
        lp_staking.resolved_lm_reward_token_amount = u64::MIN;
        lp_staking.resolved_lm_staked_token_amount = u128::MIN;
        lp_staking.current_staking_round = StakingRound::new(ctx.accounts.perpetuals.get_time()?);
        lp_staking.next_staking_round = StakingRound::new(0);
        lp_staking.resolved_staking_rounds = Vec::new();
    }

    Ok(0)
}
