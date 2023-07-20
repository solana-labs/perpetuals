use {
    anchor_lang::prelude::Pubkey,
    perpetuals::adapters::spl_governance_program_adapter,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    spl_governance::state::vote_record::{Vote, VoteChoice},
    tokio::sync::RwLock,
};

#[allow(clippy::too_many_arguments)]
pub async fn cast_vote(
    program_test_ctx: &RwLock<ProgramTestContext>,
    payer: &Keypair,
    realm_pda: &Pubkey,
    governance_pda: &Pubkey,
    proposal_pda: &Pubkey,
    governing_token_mint: &Pubkey,
    governing_token_owner: &Pubkey,
    proposal_owner: &Pubkey,
    governance_authority: &Keypair,
    approve: bool,
) -> std::result::Result<(), BanksClientError> {
    let voter_token_owner_record_address =
        spl_governance::state::token_owner_record::get_token_owner_record_address(
            &spl_governance_program_adapter::id(),
            realm_pda,
            governing_token_mint,
            governing_token_owner,
        );

    let proposal_owner_record =
        spl_governance::state::token_owner_record::get_token_owner_record_address(
            &spl_governance_program_adapter::id(),
            realm_pda,
            governing_token_mint,
            proposal_owner,
        );

    let ix = spl_governance::instruction::cast_vote(
        &spl_governance_program_adapter::id(),
        realm_pda,
        governance_pda,
        proposal_pda,
        &proposal_owner_record,
        &voter_token_owner_record_address,
        &governance_authority.pubkey(),
        governing_token_mint,
        &payer.pubkey(),
        None,
        None,
        if approve {
            Vote::Approve(vec![VoteChoice {
                rank: 0, // unused by spl governance program
                weight_percentage: 100,
            }])
        } else {
            Vote::Deny
        },
    );

    let mut ctx = program_test_ctx.write().await;
    let last_blockhash = ctx.last_blockhash;
    let banks_client = &mut ctx.banks_client;

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, governance_authority],
        last_blockhash,
    );

    banks_client.process_transaction(tx).await?;

    Ok(())
}
