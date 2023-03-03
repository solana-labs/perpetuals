use {
    crate::utils::pda,
    anchor_lang::prelude::Pubkey,
    perpetuals::adapters::spl_governance_program_adapter,
    solana_program_test::BanksClientError,
    solana_program_test::ProgramTestContext,
    solana_sdk::signer::{keypair::Keypair, Signer},
    spl_governance::state::enums::MintMaxVoterWeightSource,
    spl_governance::state::realm::GoverningTokenConfigAccountArgs,
    spl_governance::state::realm_config::GoverningTokenType,
};

pub async fn create_realm(
    program_test_ctx: &mut ProgramTestContext,
    admin: &Keypair,
    payer: &Keypair,
    name: String,
    min_community_weight_to_create_governance: u64,
    community_token_mint: &Pubkey,
) -> std::result::Result<Pubkey, BanksClientError> {
    let realm_pda = pda::get_governance_realm_pda(name.clone());

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[spl_governance::instruction::create_realm(
            &spl_governance_program_adapter::id(),
            &admin.pubkey(),
            community_token_mint,
            &payer.pubkey(),
            None,
            Some(GoverningTokenConfigAccountArgs {
                token_type: GoverningTokenType::Liquid,
                voter_weight_addin: None,
                max_voter_weight_addin: None,
            }),
            None,
            name,
            min_community_weight_to_create_governance,
            MintMaxVoterWeightSource::SupplyFraction(100),
        )],
        Some(&payer.pubkey()),
        &[payer],
        program_test_ctx.last_blockhash,
    );

    program_test_ctx
        .banks_client
        .process_transaction(tx)
        .await?;

    Ok(realm_pda)
}
