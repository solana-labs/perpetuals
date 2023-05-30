use {
    anchor_lang::{prelude::*, system_program},
    anchor_spl::token::spl_token,
    solana_program::instruction::Instruction,
    spl_governance::{
        instruction::GovernanceInstruction,
        state::{
            realm::get_governing_token_holding_address, realm_config::get_realm_config_address,
            token_owner_record::get_token_owner_record_address, vote_record::Vote,
        },
    },
};

#[derive(Clone, Copy)]
pub struct SplGovernanceV3Adapter;

pub mod spl_governance_program_adapter {
    solana_program::declare_id!("GovER5Lthms3bLBqWub97yVrMmEogzX7xNjdXpPPCVZw");
}

impl anchor_lang::Id for SplGovernanceV3Adapter {
    fn id() -> Pubkey {
        spl_governance_program_adapter::ID
    }
}

fn assert_governance_program_account(spl_governance_program: &Pubkey) -> Result<()> {
    require_eq!(
        spl_governance_program,
        &spl_governance_program_adapter::ID,
        ErrorCode::InvalidProgramId,
    );

    Ok(())
}

fn get_opt_pubkey_from_account_info(opt_acc: Option<&AccountInfo>) -> Option<Pubkey> {
    let acc = opt_acc?;

    Some(*acc.key)
}

pub fn set_governance_delegate<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, SetGovernanceDelegate<'info>>,
) -> Result<()> {
    assert_governance_program_account(ctx.program.key)?;

    let mut remaining_accounts_iter = ctx.remaining_accounts.iter();

    let ix = spl_governance::instruction::set_governance_delegate(
        &spl_governance_program_adapter::ID,
        ctx.accounts.governance_authority.key,
        ctx.accounts.realm.key,
        ctx.accounts.governing_token_mint.key,
        ctx.accounts.governing_token_owner.key,
        &get_opt_pubkey_from_account_info(remaining_accounts_iter.next()),
    );

    solana_program::program::invoke_signed(
        &ix,
        &ToAccountInfos::to_account_infos(&ctx),
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn deposit_governing_tokens<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, DepositGoverningTokens<'info>>,
    amount: u64,
) -> Result<()> {
    assert_governance_program_account(ctx.program.key)?;

    let ix = spl_governance::instruction::deposit_governing_tokens(
        &spl_governance_program_adapter::ID,
        ctx.accounts.realm.key,
        ctx.accounts.governing_token_source.key,
        ctx.accounts.governing_token_owner.key,
        ctx.accounts.governing_token_transfer_authority.key,
        ctx.accounts.payer.key,
        amount,
        ctx.accounts.governing_token_mint.key,
    );

    solana_program::program::invoke_signed(
        &ix,
        &ToAccountInfos::to_account_infos(&ctx),
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

/// Creates DepositGoverningTokens instruction
///
/// This is a declination of the base version where the owner is not a signer
/// There is a pesky check that serves no purpose in the deposit function verifying that the
/// owner of the token_owner_record is signing when the token_owner_record data is empty.
/// We bypass this by calling the `create_token_owner_record` ix first and using this altered version of the IX.
#[allow(clippy::too_many_arguments)]
pub fn deposit_governing_tokens_owner_not_signer<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, DepositGoverningTokens<'info>>,
    amount: u64,
) -> Result<()> {
    let program_id = &spl_governance_program_adapter::ID;
    let realm = ctx.accounts.realm.key;
    let governing_token_source = ctx.accounts.governing_token_source.key;
    let governing_token_owner = ctx.accounts.governing_token_owner.key;
    let governing_token_source_authority = ctx.accounts.governing_token_transfer_authority.key;
    let payer = ctx.accounts.payer.key;
    let governing_token_mint = ctx.accounts.governing_token_mint.key;

    let token_owner_record_address = get_token_owner_record_address(
        program_id,
        realm,
        governing_token_mint,
        governing_token_owner,
    );

    let governing_token_holding_address =
        get_governing_token_holding_address(program_id, realm, governing_token_mint);

    let realm_config_address = get_realm_config_address(program_id, realm);

    let accounts = vec![
        AccountMeta::new_readonly(*realm, false),
        AccountMeta::new(governing_token_holding_address, false),
        AccountMeta::new(*governing_token_source, false),
        AccountMeta::new_readonly(*governing_token_owner, false), // FASLE, not signing
        AccountMeta::new_readonly(*governing_token_source_authority, true),
        AccountMeta::new(token_owner_record_address, false),
        AccountMeta::new(*payer, true),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(spl_token::id(), false),
        AccountMeta::new_readonly(realm_config_address, false),
    ];

    let instruction = GovernanceInstruction::DepositGoverningTokens { amount };

    let solana_instruction = Instruction {
        program_id: *program_id,
        accounts,
        data: instruction.try_to_vec().unwrap(),
    };

    solana_program::program::invoke_signed(
        &solana_instruction,
        &ToAccountInfos::to_account_infos(&ctx),
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn withdraw_governing_tokens<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, WithdrawGoverningTokens<'info>>,
) -> Result<()> {
    assert_governance_program_account(ctx.program.key)?;

    let ix = spl_governance::instruction::withdraw_governing_tokens(
        &spl_governance_program_adapter::ID,
        ctx.accounts.realm.key,
        ctx.accounts.governing_token_destination.key,
        ctx.accounts.governing_token_owner.key,
        ctx.accounts.governing_token_mint.key,
    );

    solana_program::program::invoke_signed(
        &ix,
        &ToAccountInfos::to_account_infos(&ctx),
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn cast_vote<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, CastVote<'info>>,
    vote: Vote,
) -> Result<()> {
    assert_governance_program_account(ctx.program.key)?;

    let mut remaining_accounts_iter = ctx.remaining_accounts.iter();

    let ix = spl_governance::instruction::cast_vote(
        &spl_governance_program_adapter::ID,
        ctx.accounts.realm.key,
        ctx.accounts.governance.key,
        ctx.accounts.proposal.key,
        ctx.accounts.proposal_owner_record.key,
        ctx.accounts.voter_token_owner_record.key,
        ctx.accounts.governance_authority.key,
        ctx.accounts.governing_token_mint.key,
        ctx.accounts.payer.key,
        get_opt_pubkey_from_account_info(remaining_accounts_iter.next()),
        get_opt_pubkey_from_account_info(remaining_accounts_iter.next()),
        vote,
    );

    solana_program::program::invoke_signed(
        &ix,
        &ToAccountInfos::to_account_infos(&ctx),
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn relinquish_vote<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, RelinquishVote<'info>>,
) -> Result<()> {
    assert_governance_program_account(ctx.program.key)?;

    let mut remaining_accounts_iter = ctx.remaining_accounts.iter();

    let ix = spl_governance::instruction::relinquish_vote(
        &spl_governance_program_adapter::ID,
        ctx.accounts.realm.key,
        ctx.accounts.governance.key,
        ctx.accounts.proposal.key,
        ctx.accounts.voter_token_owner_record.key,
        ctx.accounts.governing_token_mint.key,
        get_opt_pubkey_from_account_info(remaining_accounts_iter.next()),
        get_opt_pubkey_from_account_info(remaining_accounts_iter.next()),
    );

    solana_program::program::invoke_signed(
        &ix,
        &ToAccountInfos::to_account_infos(&ctx),
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn revoke_governing_token<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, RevokeGoverningTokens<'info>>,
    amount: u64,
) -> Result<()> {
    assert_governance_program_account(ctx.program.key)?;

    let ix = spl_governance::instruction::revoke_governing_tokens(
        &spl_governance_program_adapter::ID,
        ctx.accounts.realm.key,
        ctx.accounts.governing_token_owner.key,
        ctx.accounts.governing_token_mint.key,
        ctx.accounts.governing_token_mint_authority.key,
        amount,
    );

    solana_program::program::invoke_signed(
        &ix,
        &ToAccountInfos::to_account_infos(&ctx),
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn create_token_owner_record<'a, 'b, 'c, 'info>(
    ctx: CpiContext<'a, 'b, 'c, 'info, CreateTokenOwnerRecord<'info>>,
) -> Result<()> {
    assert_governance_program_account(ctx.program.key)?;

    let ix = spl_governance::instruction::create_token_owner_record(
        &spl_governance_program_adapter::ID,
        ctx.accounts.realm.key,
        ctx.accounts.governing_token_owner.key,
        ctx.accounts.governing_token_mint.key,
        ctx.accounts.payer.key,
    );

    solana_program::program::invoke_signed(
        &ix,
        &ToAccountInfos::to_account_infos(&ctx),
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct SetGovernanceDelegate<'info> {
    /// CHECK: Handled by spl governance program
    pub governance_authority: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub realm: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_mint: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_owner: AccountInfo<'info>,

    // Following accounts required to be in context
    /// CHECK: Handled by spl governance program
    pub governing_token_owner_record: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct DepositGoverningTokens<'info> {
    /// CHECK: Handled by spl governance program
    pub realm: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_mint: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_source: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_owner: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_transfer_authority: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub payer: AccountInfo<'info>,

    // Following accounts required to be in context
    /// CHECK: Handled by spl governance program
    pub realm_config: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_holding: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_owner_record: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct WithdrawGoverningTokens<'info> {
    /// CHECK: Handled by spl governance program
    pub realm: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_destination: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_owner: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_mint: AccountInfo<'info>,

    // Following accounts required to be in context
    /// CHECK: Handled by spl governance program
    pub realm_config: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_holding: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_owner_record: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CastVote<'info> {
    /// CHECK: Handled by spl governance program
    pub governance: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub realm: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governance_authority: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub proposal: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub proposal_owner_record: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub voter_token_owner_record: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_authority: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_mint: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub payer: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct RelinquishVote<'info> {
    /// CHECK: Handled by spl governance program
    pub governance: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub realm: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub proposal: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub voter_token_owner_record: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_mint: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct RevokeGoverningTokens<'info> {
    /// CHECK: Handled by spl governance program
    pub realm: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_holding: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_owner_record: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_mint: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program - Can be either the mint or the owner
    pub governing_token_revoke_authority: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub realm_config: AccountInfo<'info>,

    /// CHECK: Handled by spl governance program
    pub governing_token_owner: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_mint_authority: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CreateTokenOwnerRecord<'info> {
    /// CHECK: Handled by spl governance program
    pub realm: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_owner: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_owner_record: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub governing_token_mint: AccountInfo<'info>,
    /// CHECK: Handled by spl governance program
    pub payer: AccountInfo<'info>,
}
