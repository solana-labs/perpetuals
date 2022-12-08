//! WithdrawFees instruction handler

use {
    crate::{
        math,
        state::{
            custody::Custody,
            multisig::{AdminInstruction, Multisig},
            perpetuals::Perpetuals,
            pool::Pool,
        },
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Token, TokenAccount},
    solana_program::sysvar,
};

#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account()]
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
                 custody.mint.key().as_ref()],
        bump = custody.bump
    )]
    pub custody: Box<Account<'info, Custody>>,

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
        constraint = receiving_token_account.mint == custody_token_account.mint
    )]
    pub receiving_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: SOL fees receiving account
    #[account(
        mut,
        constraint = receiving_sol_account.data_is_empty()
    )]
    pub receiving_sol_account: AccountInfo<'info>,

    token_program: Program<'info, Token>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct WithdrawFeesParams {
    pub token_amount: u64,
    pub sol_amount: u64,
}

pub fn withdraw_fees<'info>(
    ctx: Context<'_, '_, '_, 'info, WithdrawFees<'info>>,
    params: &WithdrawFeesParams,
) -> Result<u8> {
    // validate inputs
    if params.token_amount == 0 && params.sol_amount == 0 {
        return Err(ProgramError::InvalidArgument.into());
    }

    // validate signatures
    let mut multisig = ctx.accounts.multisig.load_mut()?;

    let signatures_left = multisig.sign_multisig(
        &ctx.accounts.admin,
        &Multisig::get_account_infos(&ctx)[1..],
        &Multisig::get_instruction_data(AdminInstruction::WithdrawFees, params)?,
    )?;
    if signatures_left > 0 {
        msg!(
            "Instruction has been signed but more signatures are required: {}",
            signatures_left
        );
        return Ok(signatures_left);
    }

    // transfer token fees from the custody to the receiver
    if params.token_amount > 0 {
        let custody = ctx.accounts.custody.as_mut();

        msg!(
            "Withdraw token fees: {} / {}",
            params.token_amount,
            custody.assets.protocol_fees
        );

        if custody.assets.protocol_fees < params.token_amount {
            return Err(ProgramError::InsufficientFunds.into());
        }
        custody.assets.protocol_fees =
            math::checked_sub(custody.assets.protocol_fees, params.token_amount)?;

        ctx.accounts.perpetuals.transfer_tokens(
            ctx.accounts.custody_token_account.to_account_info(),
            ctx.accounts.receiving_token_account.to_account_info(),
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            params.token_amount,
        )?;
    }

    // transfer sol fees from the custody to the receiver
    if params.sol_amount > 0 {
        let balance = ctx.accounts.transfer_authority.try_lamports()?;
        let min_balance = sysvar::rent::Rent::get().unwrap().minimum_balance(0);
        let available_balance = if balance > min_balance {
            math::checked_sub(balance, min_balance)?
        } else {
            0
        };

        msg!(
            "Withdraw SOL fees: {} / {}",
            params.sol_amount,
            available_balance
        );

        if available_balance < params.sol_amount {
            return Err(ProgramError::InsufficientFunds.into());
        }

        Perpetuals::transfer_sol_from_owned(
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.receiving_sol_account.to_account_info(),
            params.sol_amount,
        )?;
    }

    Ok(0)
}
