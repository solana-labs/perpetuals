//! SetCustodyConfig instruction handler

use {
    crate::{
        error::PerpetualsError,
        state::{
            custody::{BorrowRateParams, Custody, Fees, OracleParams, PricingParams},
            multisig::{AdminInstruction, Multisig},
            perpetuals::Permissions,
            pool::Pool,
        },
    },
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
pub struct SetCustodyConfig<'info> {
    #[account()]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"multisig"],
        bump = multisig.load()?.bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

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
                 custody.mint.as_ref()],
        bump
    )]
    pub custody: Box<Account<'info, Custody>>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SetCustodyConfigParams {
    pub is_stable: bool,
    pub oracle: OracleParams,
    pub pricing: PricingParams,
    pub permissions: Permissions,
    pub fees: Fees,
    pub borrow_rate: BorrowRateParams,
    pub target_ratio: u64,
    pub min_ratio: u64,
    pub max_ratio: u64,
}

pub fn set_custody_config<'info>(
    ctx: Context<'_, '_, '_, 'info, SetCustodyConfig<'info>>,
    params: &SetCustodyConfigParams,
) -> Result<u8> {
    // validate inputs
    if params.min_ratio > params.target_ratio || params.target_ratio > params.max_ratio {
        return Err(ProgramError::InvalidArgument.into());
    }

    // validate signatures
    let mut multisig = ctx.accounts.multisig.load_mut()?;

    let signatures_left = multisig.sign_multisig(
        &ctx.accounts.admin,
        &Multisig::get_account_infos(&ctx)[1..],
        &Multisig::get_instruction_data(AdminInstruction::SetCustodyConfig, params)?,
    )?;
    if signatures_left > 0 {
        msg!(
            "Instruction has been signed but more signatures are required: {}",
            signatures_left
        );
        return Ok(signatures_left);
    }

    // update pool data
    let pool = ctx.accounts.pool.as_mut();
    let idx = pool.get_token_id(&ctx.accounts.custody.key())?;
    pool.tokens[idx].target_ratio = params.target_ratio;
    pool.tokens[idx].min_ratio = params.min_ratio;
    pool.tokens[idx].max_ratio = params.max_ratio;

    // update custody data
    let custody = ctx.accounts.custody.as_mut();
    custody.is_stable = params.is_stable;
    custody.oracle = params.oracle;
    custody.pricing = params.pricing;
    custody.permissions = params.permissions;
    custody.fees = params.fees;
    custody.borrow_rate = params.borrow_rate;

    if !custody.validate() {
        err!(PerpetualsError::InvalidCustodyConfig)
    } else {
        Ok(0)
    }
}
