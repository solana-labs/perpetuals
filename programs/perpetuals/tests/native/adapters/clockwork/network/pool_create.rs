use {
    crate::utils::pda,
    anchor_lang::{InstructionData, ToAccountMetas},
    solana_program::instruction::Instruction,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    tokio::sync::RwLock,
};

pub async fn pool_create(
    program_test_ctx: &RwLock<ProgramTestContext>,
    admin: &Keypair,
    payer: &Keypair,
) -> std::result::Result<(), BanksClientError> {
    let config_pda = pda::get_clockwork_network_config_pda().0;
    let registry_pda = pda::get_clockwork_network_registry_pda().0;
    let pool_pda = clockwork_network_program::state::Pool::pubkey(0);

    let ix = Instruction {
        program_id: clockwork_network_program::ID,
        accounts: clockwork_network_program::accounts::PoolCreate {
            admin: admin.pubkey(),
            config: config_pda,
            payer: payer.pubkey(),
            pool: pool_pda,
            registry: registry_pda,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: clockwork_network_program::instruction::PoolCreate {}.data(),
    };

    let mut ctx = program_test_ctx.write().await;
    let last_blockhash = ctx.last_blockhash;
    let banks_client = &mut ctx.banks_client;

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, admin],
        last_blockhash,
    );

    banks_client.process_transaction(tx).await?;

    Ok(())
}
