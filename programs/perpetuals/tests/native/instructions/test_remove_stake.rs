use {
    crate::utils::{self, pda},
    anchor_lang::{
        prelude::{Clock, Pubkey},
        ToAccountMetas,
    },
    bonfida_test_utils::ProgramTestContextExt,
    perpetuals::{
        adapters::spl_governance_program_adapter,
        instructions::RemoveStakeParams,
        state::{cortex::Cortex, stake::Stake},
    },
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn test_remove_stake(
    program_test_ctx: &mut ProgramTestContext,
    owner: &Keypair,
    payer: &Keypair,
    params: RemoveStakeParams,
    stake_reward_token_mint: &Pubkey,
    governance_realm_pda: &Pubkey,
) -> std::result::Result<(), BanksClientError> {
    // ==== GIVEN =============================================================
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let stake_pda = pda::get_stake_pda(&owner.pubkey()).0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let stake_token_account_pda = pda::get_stake_token_account_pda().0;
    let stake_reward_token_account_pda = pda::get_stake_reward_token_account_pda().0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;

    let lm_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lm_token_mint_pda).0;
    let stake_reward_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &stake_reward_token_mint).0;

    let governance_governing_token_holding_pda =
        pda::get_governance_governing_token_holding_pda(governance_realm_pda, &lm_token_mint_pda);

    let governance_realm_config_pda = pda::get_governance_realm_config_pda(governance_realm_pda);

    let governance_governing_token_owner_record_pda =
        pda::get_governance_governing_token_owner_record_pda(
            governance_realm_pda,
            &lm_token_mint_pda,
            &stake_pda,
        );

    // // ==== WHEN ==============================================================
    // save account state before tx execution
    let cortex_account_before = utils::get_account::<Cortex>(program_test_ctx, cortex_pda).await;
    let stake_account_before = utils::get_account::<Stake>(program_test_ctx, stake_pda).await;
    let owner_lm_token_account_before = program_test_ctx
        .get_token_account(lm_token_account_address)
        .await
        .unwrap();

    // Before state
    let governance_governing_token_holding_balance_before =
        utils::get_token_account_balance(program_test_ctx, governance_governing_token_holding_pda)
            .await;

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::RemoveStake {
            owner: owner.pubkey(),
            lm_token_account: lm_token_account_address,
            owner_reward_token_account: stake_reward_token_account_address,
            stake_token_account: stake_token_account_pda,
            stake_reward_token_account: stake_reward_token_account_pda,
            transfer_authority: transfer_authority_pda,
            stake: stake_pda,
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            lm_token_mint: lm_token_mint_pda,
            stake_reward_token_mint: *stake_reward_token_mint,
            governance_realm: *governance_realm_pda,
            governance_realm_config: governance_realm_config_pda,
            governance_governing_token_holding: governance_governing_token_holding_pda,
            governance_governing_token_owner_record: governance_governing_token_owner_record_pda,
            governance_program: spl_governance_program_adapter::ID,
            perpetuals_program: perpetuals::ID,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::RemoveStake {
            params: RemoveStakeParams {
                amount: params.amount,
            },
        },
        Some(&payer.pubkey()),
        &[owner, payer],
    )
    .await?;

    // ==== THEN ==============================================================

    // check balance changes
    {
        let owner_lm_token_account_after = program_test_ctx
            .get_token_account(lm_token_account_address)
            .await
            .unwrap();

        assert_eq!(
            owner_lm_token_account_before.amount + params.amount,
            owner_lm_token_account_after.amount
        );
    }

    // check `Cortex` data update
    {
        let cortex_account_after = utils::get_account::<Cortex>(program_test_ctx, cortex_pda).await;
        // same amount of resolved staking rounds
        assert_eq!(
            cortex_account_after.resolved_staking_rounds.len(),
            cortex_account_before.resolved_staking_rounds.len()
        );
        // forfeited the previously staked amount for this round
        // checked in advanced test suite

        // restaked the initial amount minus the removed amount for next round
        assert_eq!(
            cortex_account_after.next_staking_round.total_stake,
            cortex_account_before.next_staking_round.total_stake - params.amount
        );

        // note: additional tests in claim test_claim.rs (which is CPIed from this call)
    }

    // check `Stake` data update
    {
        let stake_account_after =
            utils::try_get_account::<Stake>(program_test_ctx, stake_pda).await;
        // if the whole stake wasn't removed
        if let Some(s) = stake_account_after {
            assert_eq!(s.amount, stake_account_before.amount - params.amount);

            let clock = program_test_ctx.banks_client.get_sysvar::<Clock>().await?;
            assert_eq!(s.stake_time, clock.unix_timestamp);
        } else {
            assert_eq!(stake_account_before.amount - params.amount, 0)
        }

        // note: additional tests in claim test_claim.rs (which is CPIed from this call)
    }

    // Check governance accounts
    {
        let governance_governing_token_holding_balance_after = utils::get_token_account_balance(
            program_test_ctx,
            governance_governing_token_holding_pda,
        )
        .await;

        assert_eq!(
            governance_governing_token_holding_balance_before - params.amount,
            governance_governing_token_holding_balance_after
        );
    }

    Ok(())
}
