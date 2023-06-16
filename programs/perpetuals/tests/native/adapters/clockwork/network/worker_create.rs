use {
    crate::{utils, utils::pda},
    anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas},
    clockwork_network_program::state::Registry,
    solana_program::{instruction::Instruction, sysvar},
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn worker_create(
    program_test_ctx: &mut ProgramTestContext,
    authority: &Keypair,
    signatory: &Keypair,
    payer: &Keypair,
    mint: &Pubkey,
) -> std::result::Result<(), BanksClientError> {
    let (config_pda, _) = pda::get_clockwork_network_config_pda();
    let (registry_pda, _) = pda::get_clockwork_network_registry_pda();

    let registry = utils::get_account::<Registry>(program_test_ctx, registry_pda).await;

    let (worker_pda, _) = pda::get_clockwork_network_worker_pda(registry.total_workers);

    let (fee_pda, _) = pda::get_clockwork_network_fee_pda(&worker_pda);
    let (penalty_pda, _) = pda::get_clockwork_network_penalty_pda(&worker_pda);
    let (worker_tokens_ata, _) = utils::find_associated_token_account(&worker_pda, mint);

    let ix = Instruction {
        program_id: clockwork_network_program::ID,
        accounts: clockwork_network_program::accounts::WorkerCreate {
            associated_token_program: anchor_spl::associated_token::ID,
            authority: authority.pubkey(),
            config: config_pda,
            fee: fee_pda,
            penalty: penalty_pda,
            mint: *mint,
            registry: registry_pda,
            rent: sysvar::rent::ID,
            signatory: signatory.pubkey(),
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
            worker: worker_pda,
            worker_tokens: worker_tokens_ata,
        }
        .to_account_metas(None),
        data: clockwork_network_program::instruction::WorkerCreate {}.data(),
    };

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, signatory, authority],
        program_test_ctx.last_blockhash,
    );

    program_test_ctx
        .banks_client
        .process_transaction(tx)
        .await?;

    Ok(())
}
