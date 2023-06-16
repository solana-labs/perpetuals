use {
    crate::utils::pda,
    anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas},
    solana_program::instruction::Instruction,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn thread_exec(
    program_test_ctx: &mut ProgramTestContext,
    worker_pda: &Pubkey,
    payer: &Keypair,
    signatory: &Keypair,
    thread_authority: &Pubkey,
    thread_id: Vec<u8>,
) -> std::result::Result<(), BanksClientError> {
    let (fee_pda, _) = pda::get_clockwork_network_fee_pda(worker_pda);
    let pool_pda = clockwork_network_program::state::Pool::pubkey(0);
    let (thread_pda, _) = pda::get_clockwork_thread_pda(thread_authority, thread_id);

    let ix = Instruction {
        program_id: clockwork_thread_program::ID,

        accounts: clockwork_thread_program::accounts::ThreadExec {
            fee: fee_pda,
            pool: pool_pda,
            signatory: signatory.pubkey(),
            thread: thread_pda,
            worker: *worker_pda,
        }
        .to_account_metas(None),
        data: clockwork_thread_program::instruction::ThreadExec {}.data(),
    };

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer, signatory],
        program_test_ctx.last_blockhash,
    );

    program_test_ctx
        .banks_client
        .process_transaction(tx)
        .await?;

    Ok(())
}
