use {
    anchor_lang::prelude::Pubkey,
    perpetuals::adapters::spl_governance_program_adapter,
    solana_program_test::BanksClientError,
    solana_program_test::ProgramTestContext,
    solana_sdk::signer::{keypair::Keypair, Signer},
};

#[allow(clippy::too_many_arguments)]
pub async fn relinquish_vote(
    program_test_ctx: &mut ProgramTestContext,
    payer: &Keypair,
    realm_pda: &Pubkey,
    governance_pda: &Pubkey,
    proposal_pda: &Pubkey,
    governing_token_mint: &Pubkey,
    governing_token_owner: &Pubkey,
    governance_authority: &Keypair,
) -> std::result::Result<(), BanksClientError> {
    let token_owner_record_address =
        spl_governance::state::token_owner_record::get_token_owner_record_address(
            &spl_governance_program_adapter::id(),
            realm_pda,
            governing_token_mint,
            governing_token_owner,
        );

    let ix = spl_governance::instruction::relinquish_vote(
        &spl_governance_program_adapter::id(),
        realm_pda,
        governance_pda,
        proposal_pda,
        &token_owner_record_address,
        governing_token_mint,
        Some(governance_authority.pubkey()),
        Some(*governing_token_owner),
    );

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, governance_authority],
        program_test_ctx.last_blockhash,
    );

    program_test_ctx
        .banks_client
        .process_transaction(tx)
        .await?;

    Ok(())
}
