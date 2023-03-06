use {anchor_lang::prelude::*, spl_governance::state::vote_record::Vote};

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
    if opt_acc.is_none() {
        return None;
    }

    let acc = opt_acc.unwrap();

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
