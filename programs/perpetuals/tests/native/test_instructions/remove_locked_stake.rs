use {
    crate::utils::{self, pda},
    anchor_lang::{prelude::Pubkey, AnchorSerialize, ToAccountMetas},
    perpetuals::{
        adapters::spl_governance_program_adapter, instructions::RemoveLockedStakeParams,
        state::staking::Staking,
    },
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn remove_locked_stake(
    program_test_ctx: &mut ProgramTestContext,
    owner: &Keypair,
    payer: &Keypair,
    params: RemoveLockedStakeParams,
    stake_reward_token_mint: &Pubkey,
    governance_realm_pda: &Pubkey,
) -> std::result::Result<(), BanksClientError> {
    // ==== GIVEN =============================================================
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let staking_pda = pda::get_staking_pda(&owner.pubkey()).0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let stake_token_account_pda = pda::get_stake_token_account_pda().0;
    let stake_reward_token_account_pda = pda::get_stake_reward_token_account_pda().0;
    let stake_lm_reward_token_account_pda = pda::get_stake_lm_reward_token_account_pda().0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
    let governance_token_mint_pda = pda::get_governance_token_mint_pda().0;

    let lm_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lm_token_mint_pda).0;
    let stake_reward_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), stake_reward_token_mint).0;

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

    let staking_thread_authority_pda = pda::get_staking_thread_authority(&owner.pubkey()).0;

    // // ==== WHEN ==============================================================
    // save account state before tx execution
    let staking_account_before = utils::get_account::<Staking>(program_test_ctx, staking_pda).await;

    let owner_staked_token_account_before =
        utils::get_token_account_balance(program_test_ctx, lm_token_account_address).await;

    let stakes_claim_cron_thread_address = pda::get_thread_address(
        &staking_thread_authority_pda,
        staking_account_before
            .stakes_claim_cron_thread_id
            .try_to_vec()
            .unwrap(),
    );

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::RemoveLockedStake {
            owner: owner.pubkey(),
            lm_token_account: lm_token_account_address,
            owner_reward_token_account: stake_reward_token_account_address,
            stake_token_account: stake_token_account_pda,
            stake_reward_token_account: stake_reward_token_account_pda,
            stake_lm_reward_token_account: stake_lm_reward_token_account_pda,
            transfer_authority: transfer_authority_pda,
            staking: staking_pda,
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            lm_token_mint: lm_token_mint_pda,
            stake_reward_token_mint: *stake_reward_token_mint,
            governance_realm: *governance_realm_pda,
            governance_realm_config: governance_realm_config_pda,
            governance_governing_token_holding: governance_governing_token_holding_pda,
            governance_governing_token_owner_record: governance_governing_token_owner_record_pda,
            stakes_claim_cron_thread: stakes_claim_cron_thread_address,
            staking_thread_authority: staking_thread_authority_pda,
            clockwork_program: clockwork_sdk::ID,
            governance_program: spl_governance_program_adapter::ID,
            perpetuals_program: perpetuals::ID,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
            governance_token_mint: governance_token_mint_pda,
        }
        .to_account_metas(None),
        perpetuals::instruction::RemoveLockedStake {
            params: RemoveLockedStakeParams {
                locked_stake_index: params.locked_stake_index,
            },
        },
        Some(&payer.pubkey()),
        &[owner, payer],
    )
    .await?;

    // ==== THEN ==============================================================

    let staking_account_after = utils::get_account::<Staking>(program_test_ctx, staking_pda).await;

    let owner_staked_token_account_after =
        utils::get_token_account_balance(program_test_ctx, lm_token_account_address).await;

    // Check staking account
    {
        assert_eq!(
            staking_account_after.locked_stakes.len(),
            staking_account_before.locked_stakes.len() - 1,
        );
    }

    // Check owner staked token ATA balance
    {
        // Can be higher if user claimed lm rewards
        assert!(
            owner_staked_token_account_before
                + staking_account_before.locked_stakes[params.locked_stake_index].amount
                <= owner_staked_token_account_after,
        );
    }

    Ok(())
}
