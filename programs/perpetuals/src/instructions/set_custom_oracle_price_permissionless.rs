//! SetCustomOraclePricePermissionless instruction handler

use {
    crate::{
        error::PerpetualsError,
        state::{custody::Custody, oracle::CustomOracle, perpetuals::Perpetuals, pool::Pool},
    },
    anchor_lang::prelude::*,
    solana_program::{ed25519_program, instruction::Instruction, sysvar},
};

#[derive(Accounts)]
#[instruction(params: SetCustomOraclePricePermissionlessParams)]
pub struct SetCustomOraclePricePermissionless<'info> {
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
        constraint = custody.key() == params.custody_account,
        bump = custody.bump
    )]
    pub custody: Box<Account<'info, Custody>>,

    #[account(
        // Custom oracle must first be initialized by authority before permissionless updates.
        mut,
        seeds = [b"oracle_account",
                 pool.key().as_ref(),
                 custody.mint.as_ref()],
        bump
    )]
    pub oracle_account: Box<Account<'info, CustomOracle>>,

    /// CHECK: Needed for ed25519 signature verification, to inspect all instructions in this transaction.
    #[account(address = sysvar::instructions::ID)]
    pub ix_sysvar: AccountInfo<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone, PartialEq)]
pub struct SetCustomOraclePricePermissionlessParams {
    pub custody_account: Pubkey,
    pub price: u64,
    pub expo: i32,
    pub conf: u64,
    pub ema: u64,
    pub publish_time: i64,
}

pub fn set_custom_oracle_price_permissionless(
    ctx: Context<SetCustomOraclePricePermissionless>,
    params: &SetCustomOraclePricePermissionlessParams,
) -> Result<()> {
    if params.publish_time <= ctx.accounts.oracle_account.publish_time {
        msg!("Custom oracle price did not update because the requested publish time is stale.");
        return Ok(());
    }
    // Get what should be the Ed25519Program signature verification instruction.
    let signature_ix: Instruction =
        sysvar::instructions::load_instruction_at_checked(0, &ctx.accounts.ix_sysvar)?;

    validate_ed25519_signature_instruction(
        &signature_ix,
        &ctx.accounts.custody.oracle.oracle_authority,
        params,
    )?;

    ctx.accounts.oracle_account.set(
        params.price,
        params.expo,
        params.conf,
        params.ema,
        params.publish_time,
    );
    Ok(())
}

fn validate_ed25519_signature_instruction(
    signature_ix: &Instruction,
    expected_pubkey: &Pubkey,
    expected_params: &SetCustomOraclePricePermissionlessParams,
) -> Result<()> {
    require_eq!(
        signature_ix.program_id,
        ed25519_program::ID,
        PerpetualsError::PermissionlessOracleMissingSignature
    );
    require!(
        signature_ix.accounts.is_empty() /* no accounts touched */
            && signature_ix.data[0] == 0x01 /* only one ed25519 signature */
            && signature_ix.data.len() == 180, /* data len matches exactly the expected */
        PerpetualsError::PermissionlessOracleMalformedEd25519Data
    );

    // Manually access offsets for signer pubkey and message data according to:
    // https://docs.solana.com/developing/runtime-facilities/programs#ed25519-program
    let signer_pubkey = &signature_ix.data[16..16 + 32];
    let mut verified_message = &signature_ix.data[112..];

    let deserialized_instruction_params =
        SetCustomOraclePricePermissionlessParams::deserialize(&mut verified_message)?;

    require!(
        signer_pubkey == expected_pubkey.to_bytes(),
        PerpetualsError::PermissionlessOracleSignerMismatch
    );
    require!(
        deserialized_instruction_params == *expected_params,
        PerpetualsError::PermissionlessOracleMessageMismatch
    );
    Ok(())
}
