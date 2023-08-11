use {
    crate::{
        adapters,
        utils::{self, pda},
    },
    anchor_lang::{prelude::Pubkey, AnchorSerialize, ToAccountMetas},
    perpetuals::{adapters::spl_governance_program_adapter, state::user_staking::UserStaking},
    solana_program::instruction::AccountMeta,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    tokio::sync::RwLock,
};

pub async fn execute_finalize_locked_stake_thread(
    program_test_ctx: &RwLock<ProgramTestContext>,
    clockwork_worker: &Pubkey,
    clockwork_signatory: &Keypair,
    owner: &Keypair,
    payer: &Keypair,
    governance_realm_pda: &Pubkey,
    locked_stake_index: u64,
) -> std::result::Result<(), BanksClientError> {
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
    let staking_pda = pda::get_staking_pda(&lm_token_mint_pda).0;
    let user_staking_pda = pda::get_user_staking_pda(&owner.pubkey(), &staking_pda).0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let governance_token_mint_pda = pda::get_governance_token_mint_pda().0;

    let governance_governing_token_holding_pda = pda::get_governance_governing_token_holding_pda(
        governance_realm_pda,
        &governance_token_mint_pda,
    );

    let governance_realm_config_pda = pda::get_governance_realm_config_pda(governance_realm_pda);

    let governance_governing_token_owner_record_pda =
        pda::get_governance_governing_token_owner_record_pda(
            governance_realm_pda,
            &governance_token_mint_pda,
            &owner.pubkey(),
        );

    let thread_authority = pda::get_user_staking_thread_authority(&user_staking_pda).0;

    let user_staking_account =
        utils::get_account::<UserStaking>(program_test_ctx, user_staking_pda).await;

    let stake_resolution_thread_id =
        user_staking_account.locked_stakes[locked_stake_index as usize].stake_resolution_thread_id;

    let stake_resolution_thread_pda = pda::get_clockwork_thread_pda(
        &thread_authority,
        stake_resolution_thread_id.try_to_vec().unwrap(),
    )
    .0;

    adapters::clockwork::thread::thread_kickoff(
        program_test_ctx,
        clockwork_worker,
        payer,
        clockwork_signatory,
        &thread_authority,
        stake_resolution_thread_id.try_to_vec().unwrap(),
    )
    .await
    .unwrap();

    let remaining_accounts = perpetuals::accounts::FinalizeLockedStake {
        caller: stake_resolution_thread_pda,
        owner: owner.pubkey(),
        transfer_authority: transfer_authority_pda,
        user_staking: user_staking_pda,
        staking: staking_pda,
        cortex: cortex_pda,
        perpetuals: perpetuals_pda,
        lm_token_mint: lm_token_mint_pda,
        governance_realm: *governance_realm_pda,
        governance_realm_config: governance_realm_config_pda,
        governance_governing_token_holding: governance_governing_token_holding_pda,
        governance_governing_token_owner_record: governance_governing_token_owner_record_pda,
        governance_program: spl_governance_program_adapter::ID,
        perpetuals_program: perpetuals::ID,
        system_program: anchor_lang::system_program::ID,
        token_program: anchor_spl::token::ID,
        governance_token_mint: governance_token_mint_pda,
    }
    .to_account_metas(Some(false))
    .into_iter()
    .map(|x| AccountMeta {
        pubkey: x.pubkey,
        is_signer: false,
        is_writable: x.is_writable,
    })
    .collect();

    adapters::clockwork::thread::thread_exec(
        program_test_ctx,
        clockwork_worker,
        payer,
        clockwork_signatory,
        &thread_authority,
        stake_resolution_thread_id.try_to_vec().unwrap(),
        remaining_accounts,
        vec![],
    )
    .await
    .unwrap();

    let thread = utils::get_account::<clockwork_thread_program::state::Thread>(
        program_test_ctx,
        stake_resolution_thread_pda,
    )
    .await;

    println!(">>>>>>>>>>>>>>>>>> THREAD INFOS");
    println!(">>>>>>>>>>>>>>>>>> created_at: {:?}", thread.created_at);
    println!(">>>>>>>>>>>>>>>>>> exec_context: {:?}", thread.exec_context);
    println!(">>>>>>>>>>>>>>>>>> fee: {:?}", thread.fee);
    println!(">>>>>>>>>>>>>>>>>> id: {:?}", thread.id);
    println!(">>>>>>>>>>>>>>>>>> instructions: {:?}", thread.instructions);
    println!(">>>>>>>>>>>>>>>>>> name: {:?}", thread.name);
    println!(
        ">>>>>>>>>>>>>>>>>> next_instruction: {:?}",
        thread.next_instruction
    );

    Ok(())
}
