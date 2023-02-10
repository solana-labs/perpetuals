//! SetBorrowRate instruction handler

use {
    crate::state::{
        custody::Custody,
        multisig::{AdminInstruction, Multisig},
        pool::Pool,
    },
    anchor_lang::prelude::*,
};

#[derive(Accounts)]
pub struct SetBorrowRate<'info> {
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
pub struct SetBorrowRateParams {
    pub borrow_rate: u64,
    pub borrow_rate_sum: u64,
}

pub fn set_borrow_rate<'info>(
    ctx: Context<'_, '_, '_, 'info, SetBorrowRate<'info>>,
    params: &SetBorrowRateParams,
) -> Result<u8> {
    // validate signatures
    let mut multisig = ctx.accounts.multisig.load_mut()?;

    let signatures_left = multisig.sign_multisig(
        &ctx.accounts.admin,
        &Multisig::get_account_infos(&ctx)[1..],
        &Multisig::get_instruction_data(AdminInstruction::SetBorrowRate, params)?,
    )?;
    if signatures_left > 0 {
        msg!(
            "Instruction has been signed but more signatures are required: {}",
            signatures_left
        );
        return Ok(signatures_left);
    }

    // update custody data
    let custody = ctx.accounts.custody.as_mut();
    custody.borrow_rate = params.borrow_rate;
    custody.borrow_rate_sum = params.borrow_rate_sum;

    Ok(0)
}
