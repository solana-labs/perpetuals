use {
    crate::utils::{self, pda},
    anchor_lang::{prelude::Pubkey, AnchorSerialize, ToAccountMetas},
    perpetuals::{
        adapters::spl_governance_program_adapter,
        instructions::AddLockedStakeParams,
        math,
        state::{perpetuals::Perpetuals, staking::Staking},
    },
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn test_add_locked_stake(
    program_test_ctx: &mut ProgramTestContext,
    owner: &Keypair,
    payer: &Keypair,
    params: AddLockedStakeParams,
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
    let locked_stake_resolution_thread_address = pda::get_thread_address(
        &staking_thread_authority_pda,
        params.stake_resolution_thread_id.try_to_vec().unwrap(),
    );

    // // ==== WHEN ==============================================================
    // save account state before tx execution
    let staking_account_before = utils::get_account::<Staking>(program_test_ctx, staking_pda).await;
    let governance_governing_token_holding_balance_before =
        utils::get_token_account_balance(program_test_ctx, governance_governing_token_holding_pda)
            .await;
    let funding_account_before =
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
        perpetuals::accounts::AddLockedStake {
            owner: owner.pubkey(),
            funding_account: lm_token_account_address,
            owner_reward_token_account: stake_reward_token_account_address,
            stake_token_account: stake_token_account_pda,
            stake_reward_token_account: stake_reward_token_account_pda,
            transfer_authority: transfer_authority_pda,
            staking: staking_pda,
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            lm_token_mint: lm_token_mint_pda,
            governance_token_mint: governance_token_mint_pda,
            stake_reward_token_mint: *stake_reward_token_mint,
            governance_realm: *governance_realm_pda,
            governance_realm_config: governance_realm_config_pda,
            governance_governing_token_holding: governance_governing_token_holding_pda,
            governance_governing_token_owner_record: governance_governing_token_owner_record_pda,
            stake_resolution_thread: locked_stake_resolution_thread_address,
            stakes_claim_cron_thread: stakes_claim_cron_thread_address,
            staking_thread_authority: staking_thread_authority_pda,
            clockwork_program: clockwork_sdk::ID,
            governance_program: spl_governance_program_adapter::ID,
            perpetuals_program: perpetuals::ID,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::AddLockedStake {
            params: AddLockedStakeParams {
                stake_resolution_thread_id: params.stake_resolution_thread_id,
                amount: params.amount,
                locked_days: params.locked_days,
            },
        },
        Some(&payer.pubkey()),
        &[owner, payer],
    )
    .await?;

    // ==== THEN ==============================================================
    let governance_governing_token_holding_balance_after =
        utils::get_token_account_balance(program_test_ctx, governance_governing_token_holding_pda)
            .await;

    let staking_account_after = utils::get_account::<Staking>(program_test_ctx, staking_pda).await;

    let funding_account_after =
        utils::get_token_account_balance(program_test_ctx, lm_token_account_address).await;

    // Check changes in staking account
    {
        assert_eq!(
            staking_account_after.locked_stakes.len(),
            staking_account_before.locked_stakes.len() + 1
        );
    }

    // Check staked token ATA balance
    {
        assert_eq!(
            funding_account_before - params.amount,
            funding_account_after,
        );
    }

    // Check voting power
    {
        // Depending on the lock duration, vote multiplier will differ
        let staking_option = staking_account_after
            .get_locked_staking_option(params.locked_days)
            .unwrap();

        let additional_voting_power = math::checked_as_u64(
            math::checked_div(
                math::checked_mul(params.amount, staking_option.vote_multiplier as u64).unwrap()
                    as u128,
                Perpetuals::BPS_POWER,
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            governance_governing_token_holding_balance_before + additional_voting_power,
            governance_governing_token_holding_balance_after,
        );
    }

    Ok(())
}
