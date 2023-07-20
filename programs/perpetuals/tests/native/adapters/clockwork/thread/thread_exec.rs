use {
    crate::utils::pda,
    anchor_lang::{prelude::Pubkey, InstructionData, ToAccountMetas},
    solana_program::instruction::{AccountMeta, Instruction},
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::{
        compute_budget::ComputeBudgetInstruction,
        signer::{keypair::Keypair, Signer},
    },
    tokio::sync::RwLock,
};

#[allow(clippy::too_many_arguments)]
pub async fn thread_exec(
    program_test_ctx: &RwLock<ProgramTestContext>,
    worker_pda: &Pubkey,
    payer: &Keypair,
    signatory: &Keypair,
    thread_authority: &Pubkey,
    thread_id: Vec<u8>,
    remaining_accounts: Vec<AccountMeta>,
    extra_signers: Vec<&Keypair>,
) -> std::result::Result<(), BanksClientError> {
    let fee_pda = pda::get_clockwork_network_fee_pda(worker_pda).0;
    let pool_pda = clockwork_network_program::state::Pool::pubkey(0);
    let thread_pda = pda::get_clockwork_thread_pda(thread_authority, thread_id).0;

    let mut accounts_meta = clockwork_thread_program::accounts::ThreadExec {
        fee: fee_pda,
        pool: pool_pda,
        signatory: signatory.pubkey(),
        thread: thread_pda,
        worker: *worker_pda,
    }
    .to_account_metas(None);

    for remaining_account in remaining_accounts {
        accounts_meta.push(remaining_account);
    }

    let ix = Instruction {
        program_id: clockwork_thread_program::ID,
        accounts: accounts_meta,
        data: clockwork_thread_program::instruction::ThreadExec {}.data(),
    };

    let mut ctx = program_test_ctx.write().await;
    let last_blockhash = ctx.last_blockhash;
    let banks_client = &mut ctx.banks_client;

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_600_000u32),
            ix,
        ],
        Some(&payer.pubkey()),
        &[&[payer, signatory], extra_signers.as_slice()].concat(),
        last_blockhash,
    );

    banks_client.process_transaction(tx).await?;

    Ok(())
}
