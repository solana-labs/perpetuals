use {
    anchor_lang::prelude::Pubkey,
    perpetuals::adapters::spl_governance_program_adapter,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    tokio::sync::RwLock,
};

#[allow(clippy::too_many_arguments)]
pub async fn create_token_owner_record(
    program_test_ctx: &RwLock<ProgramTestContext>,
    payer: &Keypair,
    realm_pda: &Pubkey,
    governing_token_mint: &Pubkey,
    governing_token_owner: &Pubkey,
) -> std::result::Result<(), BanksClientError> {
    let ix = spl_governance::instruction::create_token_owner_record(
        &spl_governance_program_adapter::id(),
        realm_pda,
        governing_token_owner,
        governing_token_mint,
        &payer.pubkey(),
    );

    let mut ctx = program_test_ctx.write().await;
    let last_blockhash = ctx.last_blockhash;
    let banks_client = &mut ctx.banks_client;

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer],
        last_blockhash,
    );

    banks_client.process_transaction(tx).await?;

    Ok(())
}
