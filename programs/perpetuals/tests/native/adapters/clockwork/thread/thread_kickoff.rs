use {
    crate::{utils, utils::pda},
    anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas},
    solana_program::instruction::Instruction,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn thread_kickoff(
    program_test_ctx: &mut ProgramTestContext,
    worker_pda: &Pubkey,
    payer: &Keypair,
    signatory: &Keypair,
    thread_authority: &Pubkey,
    thread_id: Vec<u8>,
) -> std::result::Result<(), BanksClientError> {
    let thread_pda = pda::get_clockwork_thread_pda(thread_authority, thread_id).0;

    let thread =
        utils::get_account::<clockwork_thread_program::state::Thread>(program_test_ctx, thread_pda)
            .await;

    println!("thread: {:?}", thread);

    let ix = Instruction {
        program_id: clockwork_thread_program::ID,

        accounts: clockwork_thread_program::accounts::ThreadKickoff {
            signatory: signatory.pubkey(),
            thread: thread_pda,
            worker: *worker_pda,
        }
        .to_account_metas(None),
        data: clockwork_thread_program::instruction::ThreadKickoff {}.data(),
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

// Original Time: epoch = 897, timestamp = 1689860597
// New Time: epoch = 912, timestamp = 1689882197
