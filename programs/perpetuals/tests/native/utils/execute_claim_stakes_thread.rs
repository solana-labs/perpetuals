use {
    crate::{
        adapters,
        utils::{self, pda},
    },
    anchor_lang::{prelude::Pubkey, AnchorSerialize, ToAccountMetas},
    perpetuals::state::user_staking::UserStaking,
    solana_program::instruction::AccountMeta,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    tokio::sync::RwLock,
};

// return true if executed, false if not
pub async fn execute_claim_stakes_thread(
    program_test_ctx: &RwLock<ProgramTestContext>,
    clockwork_worker: &Pubkey,

    // Pay for ClaimStakes fees
    clockwork_signatory: &Keypair,

    owner: &Keypair,

    // Pay for thread_kickoff and thread_exec fees
    payer: &Keypair,
    staking_reward_token_mint: &Pubkey,
) -> std::result::Result<bool, BanksClientError> {
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
    let staking_pda = pda::get_staking_pda(&lm_token_mint_pda).0;
    let user_staking_pda = pda::get_user_staking_pda(&owner.pubkey(), &staking_pda).0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let reward_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), staking_reward_token_mint).0;
    let lm_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lm_token_mint_pda).0;
    let staking_reward_token_vault_pda = pda::get_staking_reward_token_vault_pda(&staking_pda).0;
    let staking_lm_reward_token_vault_pda =
        pda::get_staking_lm_reward_token_vault_pda(&staking_pda).0;
    let thread_authority = pda::get_user_staking_thread_authority(&user_staking_pda).0;
    let user_staking_account =
        utils::get_account::<UserStaking>(program_test_ctx, user_staking_pda).await;

    let stakes_claim_cron_thread_address = pda::get_thread_address(
        &thread_authority,
        user_staking_account
            .stakes_claim_cron_thread_id
            .try_to_vec()
            .unwrap(),
    );

    let kickoff_result = adapters::clockwork::thread::thread_kickoff(
        program_test_ctx,
        clockwork_worker,
        payer,
        clockwork_signatory,
        &thread_authority,
        user_staking_account
            .stakes_claim_cron_thread_id
            .try_to_vec()
            .unwrap(),
    )
    .await;

    // Cron is not ready to be triggered
    if kickoff_result.is_err() {
        return Ok(false);
    }

    let remaining_accounts = perpetuals::accounts::ClaimStakes {
        caller: stakes_claim_cron_thread_address,
        owner: owner.pubkey(),
        // Payer has been set as CLOCKWORK_PAYER_PUBKEY during cron setup
        // but will be replaced dynamically by the signatory
        // needs to use signatory here
        payer: clockwork_signatory.pubkey(),
        reward_token_account: reward_token_account_address,
        lm_token_account: lm_token_account_address,
        staking_reward_token_vault: staking_reward_token_vault_pda,
        staking_lm_reward_token_vault: staking_lm_reward_token_vault_pda,
        transfer_authority: transfer_authority_pda,
        user_staking: user_staking_pda,
        staking: staking_pda,
        cortex: cortex_pda,
        perpetuals: perpetuals_pda,
        lm_token_mint: lm_token_mint_pda,
        staking_reward_token_mint: *staking_reward_token_mint,
        perpetuals_program: perpetuals::ID,
        system_program: anchor_lang::system_program::ID,
        token_program: anchor_spl::token::ID,
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
        user_staking_account
            .stakes_claim_cron_thread_id
            .try_to_vec()
            .unwrap(),
        remaining_accounts,
        vec![],
    )
    .await
    .unwrap();

    Ok(true)
}
