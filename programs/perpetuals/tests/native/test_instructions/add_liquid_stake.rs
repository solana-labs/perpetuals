use {
    crate::utils::{self, pda},
    anchor_lang::{prelude::Pubkey, AnchorSerialize, ToAccountMetas},
    perpetuals::{
        adapters::spl_governance_program_adapter,
        instructions::AddLiquidStakeParams,
        state::{
            staking::{Staking, StakingType},
            user_staking::UserStaking,
        },
    },
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    tokio::sync::RwLock,
};

pub async fn add_liquid_stake(
    program_test_ctx: &RwLock<ProgramTestContext>,
    owner: &Keypair,
    payer: &Keypair,
    params: AddLiquidStakeParams,
    governance_realm_pda: &Pubkey,
    staked_token_mint: &Pubkey,
) -> std::result::Result<(), BanksClientError> {
    // ==== GIVEN =============================================================
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let staking_pda = pda::get_staking_pda(staked_token_mint).0;
    let user_staking_pda = pda::get_user_staking_pda(&owner.pubkey(), &staking_pda).0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let staking_staked_token_vault_pda = pda::get_staking_staked_token_vault_pda(&staking_pda).0;
    let staking_reward_token_vault_pda = pda::get_staking_reward_token_vault_pda(&staking_pda).0;
    let staking_lm_reward_token_vault_pda =
        pda::get_staking_lm_reward_token_vault_pda(&staking_pda).0;
    let governance_token_mint_pda = pda::get_governance_token_mint_pda().0;

    let funding_account_address =
        utils::find_associated_token_account(&owner.pubkey(), staked_token_mint).0;

    let staking_account = utils::get_account::<Staking>(program_test_ctx, staking_pda).await;

    let reward_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &staking_account.reward_token_mint).0;
    let lm_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lm_token_mint_pda).0;

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

    let user_staking_thread_authority_pda =
        pda::get_user_staking_thread_authority(&user_staking_pda).0;

    // // ==== WHEN ==============================================================
    // save account state before tx execution
    let user_staking_account_before =
        utils::get_account::<UserStaking>(program_test_ctx, user_staking_pda).await;
    let governance_governing_token_holding_balance_before =
        utils::get_token_account_balance(program_test_ctx, governance_governing_token_holding_pda)
            .await;
    let funding_account_before =
        utils::get_token_account_balance(program_test_ctx, funding_account_address).await;

    let stakes_claim_cron_thread_address = pda::get_thread_address(
        &user_staking_thread_authority_pda,
        user_staking_account_before
            .stakes_claim_cron_thread_id
            .try_to_vec()
            .unwrap(),
    );

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::AddLiquidStake {
            owner: owner.pubkey(),
            funding_account: funding_account_address,
            reward_token_account: reward_token_account_address,
            lm_token_account: lm_token_account_address,
            staking_staked_token_vault: staking_staked_token_vault_pda,
            staking_reward_token_vault: staking_reward_token_vault_pda,
            staking_lm_reward_token_vault: staking_lm_reward_token_vault_pda,
            transfer_authority: transfer_authority_pda,
            user_staking: user_staking_pda,
            staking: staking_pda,
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            lm_token_mint: lm_token_mint_pda,
            governance_token_mint: governance_token_mint_pda,
            staking_reward_token_mint: staking_account.reward_token_mint,
            governance_realm: *governance_realm_pda,
            governance_realm_config: governance_realm_config_pda,
            governance_governing_token_holding: governance_governing_token_holding_pda,
            governance_governing_token_owner_record: governance_governing_token_owner_record_pda,
            stakes_claim_cron_thread: stakes_claim_cron_thread_address,
            user_staking_thread_authority: user_staking_thread_authority_pda,
            clockwork_program: clockwork_sdk::ID,
            governance_program: spl_governance_program_adapter::ID,
            perpetuals_program: perpetuals::ID,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::AddLiquidStake {
            params: AddLiquidStakeParams {
                amount: params.amount,
            },
        },
        Some(&payer.pubkey()),
        &[owner, payer],
        None,
        None,
    )
    .await?;

    // ==== THEN ==============================================================
    let governance_governing_token_holding_balance_after =
        utils::get_token_account_balance(program_test_ctx, governance_governing_token_holding_pda)
            .await;

    let user_staking_account_after =
        utils::get_account::<UserStaking>(program_test_ctx, user_staking_pda).await;

    let funding_account_after =
        utils::get_token_account_balance(program_test_ctx, funding_account_address).await;

    // Check changes in staking account
    {
        assert!(
            user_staking_account_after.liquid_stake.amount
                > user_staking_account_before.liquid_stake.amount,
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
        if staking_account.staking_type == StakingType::LM {
            assert_eq!(
                governance_governing_token_holding_balance_before + params.amount,
                governance_governing_token_holding_balance_after,
            );
        }
    }

    Ok(())
}
