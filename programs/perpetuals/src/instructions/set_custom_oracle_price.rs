//! SetCustomOraclePrice instruction handler

use {
    crate::state::{
        custody::Custody,
        multisig::{AdminInstruction, Multisig},
        oracle::CustomOracle,
        perpetuals::Perpetuals,
        pool::Pool,
    },
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
pub struct SetCustomOraclePrice<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        seeds = [b"multisig"],
        bump = multisig.load()?.bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

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

    #[account(
        init_if_needed,
        payer = admin,
        space = CustomOracle::LEN,
        //constraint = oracle_account.key() == custody.oracle.oracle_account,
        seeds = [b"oracle_account",
                 pool.key().as_ref(),
                 custody.mint.as_ref()],
        bump
    )]
    pub oracle_account: Box<Account<'info, CustomOracle>>,

    system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone)]
pub struct SetCustomOraclePriceParams {
    pub price: u64,
    pub expo: i32,
    pub conf: u64,
    pub ema: u64,
    pub publish_time: i64,
}

pub fn set_custom_oracle_price<'info>(
    ctx: Context<'_, '_, '_, 'info, SetCustomOraclePrice<'info>>,
    params: &SetCustomOraclePriceParams,
) -> Result<u8> {
    // validate signatures
    let mut multisig = ctx.accounts.multisig.load_mut()?;

    let signatures_left = multisig.sign_multisig(
        &ctx.accounts.admin,
        &Multisig::get_account_infos(&ctx)[1..],
        &Multisig::get_instruction_data(AdminInstruction::SetCustomOraclePrice, params)?,
    )?;
    if signatures_left > 0 {
        msg!(
            "Instruction has been signed but more signatures are required: {}",
            signatures_left
        );
        return Ok(signatures_left);
    }

    // update oracle data
    ctx.accounts.oracle_account.set(
        params.price,
        params.expo,
        params.conf,
        params.ema,
        params.publish_time,
    );
    Ok(0)
}
