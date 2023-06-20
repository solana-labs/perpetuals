use {
    crate::{
        adapters,
        utils::{self, pda},
    },
    anchor_lang::{prelude::Pubkey, AnchorSerialize, ToAccountMetas},
    perpetuals::state::staking::Staking,
    solana_program::instruction::AccountMeta,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
};

// return true if executed, false if not
pub async fn execute_claim_stakes_thread(
    program_test_ctx: &mut ProgramTestContext,
    clockwork_worker: &Pubkey,

    // Pay for ClaimStakes fees
    clockwork_signatory: &Keypair,

    owner: &Keypair,

    // Pay for thread_kickoff and thread_exec fees
    payer: &Keypair,
    stake_reward_token_mint: &Pubkey,
) -> std::result::Result<bool, BanksClientError> {
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let staking_pda = pda::get_staking_pda(&owner.pubkey()).0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
    let owner_reward_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), stake_reward_token_mint).0;
    let owner_lm_reward_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lm_token_mint_pda).0;
    let stake_reward_token_account_pda = pda::get_stake_reward_token_account_pda().0;
    let stake_lm_reward_token_account_pda = pda::get_stake_lm_reward_token_account_pda().0;
    let thread_authority = pda::get_staking_thread_authority(&owner.pubkey()).0;
    let staking_account = utils::get_account::<Staking>(program_test_ctx, staking_pda).await;

    let stakes_claim_cron_thread_address = pda::get_thread_address(
        &thread_authority,
        staking_account
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
        staking_account
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
        owner_reward_token_account: owner_reward_token_account_address,
        owner_lm_reward_token_account: owner_lm_reward_token_account_address,
        stake_reward_token_account: stake_reward_token_account_pda,
        stake_lm_reward_token_account: stake_lm_reward_token_account_pda,
        transfer_authority: transfer_authority_pda,
        staking: staking_pda,
        cortex: cortex_pda,
        perpetuals: perpetuals_pda,
        lm_token_mint: lm_token_mint_pda,
        stake_reward_token_mint: *stake_reward_token_mint,
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
        staking_account
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
