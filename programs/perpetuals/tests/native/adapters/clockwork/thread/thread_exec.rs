use {
    crate::utils::pda,
    anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas},
    solana_program::instruction::{AccountMeta, Instruction},
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
    remaining_accounts: Vec<AccountMeta>,
    extra_signers: Vec<&Keypair>,
) -> std::result::Result<(), BanksClientError> {
    let (fee_pda, _) = pda::get_clockwork_network_fee_pda(worker_pda);
    let pool_pda = clockwork_network_program::state::Pool::pubkey(0);
    let (thread_pda, _) = pda::get_clockwork_thread_pda(thread_authority, thread_id);

    let mut accounts_meta = clockwork_thread_program::accounts::ThreadExec {
        fee: fee_pda,
        pool: pool_pda,
        signatory: signatory.pubkey(),
        thread: thread_pda,
        worker: *worker_pda,
    }
    .to_account_metas(None);

    for ele in accounts_meta.as_slice() {
        println!("ACCOUNT: {:?}", ele);
    }

    for remaining_account in remaining_accounts {
        println!("REMAINING ACCOUNT: {:?}", remaining_account);

        accounts_meta.push(remaining_account);
    }

    let ix = Instruction {
        program_id: clockwork_thread_program::ID,

        accounts: accounts_meta,
        data: clockwork_thread_program::instruction::ThreadExec {}.data(),
    };

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&[payer, signatory], extra_signers.as_slice()].concat(),
        program_test_ctx.last_blockhash,
    );

    program_test_ctx
        .banks_client
        .process_transaction(tx)
        .await?;

    Ok(())
}
