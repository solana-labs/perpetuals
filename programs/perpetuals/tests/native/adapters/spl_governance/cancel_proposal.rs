use {
    anchor_lang::prelude::Pubkey,
    perpetuals::adapters::spl_governance_program_adapter,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
};

#[allow(clippy::too_many_arguments)]
pub async fn cancel_proposal(
    program_test_ctx: &mut ProgramTestContext,
    payer: &Keypair,
    realm_pda: &Pubkey,
    governance_pda: &Pubkey,
    proposal_pda: &Pubkey,
    governing_token_mint: &Pubkey,
    proposal_owner: &Pubkey,
    governance_authority: &Keypair,
) -> std::result::Result<(), BanksClientError> {
    let proposal_owner_record =
        spl_governance::state::token_owner_record::get_token_owner_record_address(
            &spl_governance_program_adapter::id(),
            realm_pda,
            governing_token_mint,
            proposal_owner,
        );

    let ix = spl_governance::instruction::cancel_proposal(
        &spl_governance_program_adapter::id(),
        realm_pda,
        governance_pda,
        proposal_pda,
        &proposal_owner_record,
        &governance_authority.pubkey(),
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
