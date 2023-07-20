use {
    crate::utils::{pda, utils},
    anchor_lang::prelude::Pubkey,
    perpetuals::adapters::spl_governance_program_adapter,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    spl_governance::state::{
        enums::MintMaxVoterWeightSource,
        realm::{GoverningTokenConfigAccountArgs, RealmV2},
        realm_config::GoverningTokenType,
    },
    tokio::sync::RwLock,
};

pub async fn create_realm(
    program_test_ctx: &RwLock<ProgramTestContext>,
    admin: &Keypair,
    payer: &Keypair,
    name: String,
    min_community_weight_to_create_governance: u64,
    community_token_mint: &Pubkey,
) -> std::result::Result<Pubkey, BanksClientError> {
    let realm_pda = pda::get_governance_realm_pda(name.clone());

    let mut ctx: tokio::sync::RwLockWriteGuard<'_, ProgramTestContext> =
        program_test_ctx.write().await;
    let last_blockhash = ctx.last_blockhash;
    let banks_client = &mut ctx.banks_client;

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[spl_governance::instruction::create_realm(
            &spl_governance_program_adapter::id(),
            &admin.pubkey(),
            community_token_mint,
            &payer.pubkey(),
            None,
            Some(GoverningTokenConfigAccountArgs {
                token_type: GoverningTokenType::Membership,
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
        last_blockhash,
    );

    banks_client.process_transaction(tx).await?;

    drop(ctx);

    {
        let realm = utils::get_borsh_account::<RealmV2>(program_test_ctx, &realm_pda).await;

        assert_eq!(realm.community_mint, *community_token_mint);
        assert_eq!(realm.authority.unwrap(), admin.pubkey());
    }

    Ok(realm_pda)
}
