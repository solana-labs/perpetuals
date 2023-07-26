use {
    crate::{
        test_instructions,
        utils::{self, pda},
    },
    maplit::hashmap,
    perpetuals::{
        instructions::{
            AddLiquidStakeParams, AddLiquidityParams, AddVestParams, RemoveLiquidStakeParams,
        },
        state::{cortex::Cortex, perpetuals::Perpetuals, staking::StakingRound},
    },
    solana_sdk::signer::Signer,
};

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;

pub async fn liquid_staking() {
    let test_setup = utils::TestSetup::new(
        vec![
            utils::UserParam {
                name: "alice",
                token_balances: hashmap! {
                    "usdc" => utils::scale(3_000, USDC_DECIMALS),
                    "eth" => utils::scale(2, ETH_DECIMALS),
                },
            },
            utils::UserParam {
                name: "martin",
                token_balances: hashmap! {
                    "usdc" => utils::scale(3_000, USDC_DECIMALS),
                    "eth" => utils::scale(2, ETH_DECIMALS),
                },
            },
        ],
        vec![
            utils::MintParam {
                name: "usdc",
                decimals: USDC_DECIMALS,
            },
            utils::MintParam {
                name: "eth",
                decimals: ETH_DECIMALS,
            },
        ],
        vec!["admin_a", "admin_b", "admin_c"],
        "usdc",
        "usdc",
        6,
        "ADRENA",
        "main_pool",
        vec![
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
                    mint_name: "usdc",
                    is_stable: true,
                    is_virtual: false,
                    target_ratio: utils::ratio_from_percentage(50.0),
                    min_ratio: utils::ratio_from_percentage(0.0),
                    max_ratio: utils::ratio_from_percentage(100.0),
                    initial_price: utils::scale(1, USDC_DECIMALS),
                    initial_conf: utils::scale_f64(0.01, USDC_DECIMALS),
                    pricing_params: None,
                    permissions: None,
                    fees: None,
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale(1_500, USDC_DECIMALS),
                payer_user_name: "alice",
            },
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
                    mint_name: "eth",
                    is_stable: false,
                    is_virtual: false,
                    target_ratio: utils::ratio_from_percentage(50.0),
                    min_ratio: utils::ratio_from_percentage(0.0),
                    max_ratio: utils::ratio_from_percentage(100.0),
                    initial_price: utils::scale(1_500, ETH_DECIMALS),
                    initial_conf: utils::scale(10, ETH_DECIMALS),
                    pricing_params: None,
                    permissions: None,
                    fees: None,
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale(1, ETH_DECIMALS),
                payer_user_name: "alice",
            },
        ],
        utils::scale(1_000_000, Cortex::LM_DECIMALS),
        utils::scale(1_000_000, Cortex::LM_DECIMALS),
        utils::scale(1_000_000, Cortex::LM_DECIMALS),
        utils::scale(1_000_000, Cortex::LM_DECIMALS),
    )
    .await;

    let alice = test_setup.get_user_keypair_by_name("alice");
    let martin = test_setup.get_user_keypair_by_name("martin");

    let admin_a = test_setup.get_multisig_member_keypair_by_name("admin_a");

    let cortex_stake_reward_mint = test_setup.get_cortex_stake_reward_mint();
    let multisig_signers = test_setup.get_multisig_signers();

    let eth_mint = &test_setup.get_mint_by_name("eth");

    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;

    // Prep work: Alice get 2 governance tokens using vesting
    {
        let current_time = utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await;

        test_instructions::add_vest(
            &test_setup.program_test_ctx,
            admin_a,
            &test_setup.payer_keypair,
            alice,
            &test_setup.governance_realm_pda,
            &AddVestParams {
                amount: utils::scale(2, Cortex::LM_DECIMALS),
                unlock_start_timestamp: current_time,
                unlock_end_timestamp: current_time + utils::days_in_seconds(7),
            },
            &multisig_signers,
        )
        .await
        .unwrap();

        // Move until vest end
        utils::warp_forward(&test_setup.program_test_ctx, utils::days_in_seconds(7)).await;

        test_instructions::claim_vest(
            &test_setup.program_test_ctx,
            &test_setup.payer_keypair,
            alice,
            &test_setup.governance_realm_pda,
        )
        .await
        .unwrap();
    }

    let alice_staking_reward_token_account_address =
        utils::find_associated_token_account(&alice.pubkey(), &cortex_stake_reward_mint).0;

    let alice_lm_token_account_address =
        utils::find_associated_token_account(&alice.pubkey(), &lm_token_mint_pda).0;

    let stakes_claim_cron_thread_id =
        utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await as u64;

    {
        // LM Staking
        test_instructions::init_user_staking(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            &lm_token_mint_pda,
            perpetuals::instructions::InitUserStakingParams {
                stakes_claim_cron_thread_id,
            },
        )
        .await
        .unwrap();

        // LP Staking
        test_instructions::init_user_staking(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            &test_setup.lp_token_mint_pda,
            perpetuals::instructions::InitUserStakingParams {
                stakes_claim_cron_thread_id,
            },
        )
        .await
        .unwrap();
    }

    // Alice: add LM liquid staking
    {
        test_instructions::add_liquid_stake(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            AddLiquidStakeParams {
                amount: utils::scale(1, Cortex::LM_DECIMALS),
            },
            &test_setup.governance_realm_pda,
            &lm_token_mint_pda,
        )
        .await
        .unwrap();
    }

    // Alice: add LP liquid staking
    {
        test_instructions::add_liquid_stake(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            AddLiquidStakeParams {
                amount: utils::scale(1, Perpetuals::LP_DECIMALS),
            },
            &test_setup.governance_realm_pda,
            &test_setup.lp_token_mint_pda,
        )
        .await
        .unwrap();
    }

    utils::warp_forward(&test_setup.program_test_ctx, 1).await;

    // Alice: claim when there is nothing to claim yet
    {
        let balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_staking_reward_token_account_address,
        )
        .await;

        let lm_balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_lm_token_account_address,
        )
        .await;

        test_instructions::claim_stakes(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &lm_token_mint_pda,
        )
        .await
        .unwrap();

        test_instructions::claim_stakes(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &test_setup.lp_token_mint_pda,
        )
        .await
        .unwrap();

        let balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_staking_reward_token_account_address,
        )
        .await;

        let lm_balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_lm_token_account_address,
        )
        .await;

        assert_eq!(balance_before, balance_after);
        assert_eq!(lm_balance_before, lm_balance_after);
    }

    // warp to the next round and resolve the current one
    // this round bear no rewards for the new staking at the staking started during the round
    {
        utils::warp_forward(
            &test_setup.program_test_ctx,
            StakingRound::ROUND_MIN_DURATION_SECONDS,
        )
        .await;

        test_instructions::resolve_staking_round(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &lm_token_mint_pda,
        )
        .await
        .unwrap();

        test_instructions::resolve_staking_round(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &test_setup.lp_token_mint_pda,
        )
        .await
        .unwrap();
    }

    // warp to the next round and resolve the current one
    // this round bear rewards for the liquid stake
    {
        utils::warp_forward(
            &test_setup.program_test_ctx,
            StakingRound::ROUND_MIN_DURATION_SECONDS,
        )
        .await;

        test_instructions::resolve_staking_round(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &lm_token_mint_pda,
        )
        .await
        .unwrap();

        test_instructions::resolve_staking_round(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &test_setup.lp_token_mint_pda,
        )
        .await
        .unwrap();
    }

    // Claim when there is one round worth of rewards to claim
    {
        let balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_staking_reward_token_account_address,
        )
        .await;

        let lm_balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_lm_token_account_address,
        )
        .await;

        test_instructions::claim_stakes(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &lm_token_mint_pda,
        )
        .await
        .unwrap();

        test_instructions::claim_stakes(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &test_setup.lp_token_mint_pda,
        )
        .await
        .unwrap();

        let balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_staking_reward_token_account_address,
        )
        .await;

        let lm_balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_lm_token_account_address,
        )
        .await;

        assert_eq!(balance_after - balance_before, 35_466_511);
        assert_eq!(lm_balance_after - lm_balance_before, 2_000_000);
    }

    // warp to the next round and resolve the current one
    {
        utils::warp_forward(
            &test_setup.program_test_ctx,
            StakingRound::ROUND_MIN_DURATION_SECONDS,
        )
        .await;

        test_instructions::resolve_staking_round(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &lm_token_mint_pda,
        )
        .await
        .unwrap();

        test_instructions::resolve_staking_round(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &test_setup.lp_token_mint_pda,
        )
        .await
        .unwrap();
    }

    // Use add liquidity to generate rewards for the current round
    {
        // Generate platform activity to fill current round' rewards
        test_instructions::add_liquidity(
            &test_setup.program_test_ctx,
            martin,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            eth_mint,
            AddLiquidityParams {
                amount_in: utils::scale_f64(0.25, ETH_DECIMALS),
                min_lp_amount_out: 1,
            },
        )
        .await
        .unwrap();
    }

    // Alice: add LM liquid stake when staking is already pending
    test_instructions::add_liquid_stake(
        &test_setup.program_test_ctx,
        alice,
        &test_setup.payer_keypair,
        AddLiquidStakeParams {
            amount: utils::scale(1, Cortex::LM_DECIMALS),
        },
        &test_setup.governance_realm_pda,
        &lm_token_mint_pda,
    )
    .await
    .unwrap();

    utils::warp_forward(&test_setup.program_test_ctx, 1).await;

    // warp to the next round and resolve the current one
    {
        utils::warp_forward(
            &test_setup.program_test_ctx,
            StakingRound::ROUND_MIN_DURATION_SECONDS,
        )
        .await;

        test_instructions::resolve_staking_round(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &lm_token_mint_pda,
        )
        .await
        .unwrap();

        test_instructions::resolve_staking_round(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &test_setup.lp_token_mint_pda,
        )
        .await
        .unwrap();
    }

    // Claim rewards - alice should get rewards from her first liquid staking for this round
    // New staking should start accruing rewards next round only
    {
        let balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_staking_reward_token_account_address,
        )
        .await;

        let lm_balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_lm_token_account_address,
        )
        .await;

        test_instructions::claim_stakes(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &lm_token_mint_pda,
        )
        .await
        .unwrap();

        test_instructions::claim_stakes(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &test_setup.lp_token_mint_pda,
        )
        .await
        .unwrap();

        let balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_staking_reward_token_account_address,
        )
        .await;

        let lm_balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_lm_token_account_address,
        )
        .await;

        assert_eq!(balance_after - balance_before, 2_710_419);
        assert_eq!(lm_balance_after - lm_balance_before, 2_000_000);
    }

    // Remove half the stake
    test_instructions::remove_liquid_stake(
        &test_setup.program_test_ctx,
        alice,
        &test_setup.payer_keypair,
        RemoveLiquidStakeParams {
            amount: utils::scale(1, Cortex::LM_DECIMALS),
        },
        &cortex_stake_reward_mint,
        &test_setup.governance_realm_pda,
    )
    .await
    .unwrap();

    utils::warp_forward(&test_setup.program_test_ctx, 1).await;

    // Try & remove more than possible should fail
    assert!(test_instructions::remove_liquid_stake(
        &test_setup.program_test_ctx,
        alice,
        &test_setup.payer_keypair,
        RemoveLiquidStakeParams {
            amount: utils::scale(42, Cortex::LM_DECIMALS),
        },
        &cortex_stake_reward_mint,
        &test_setup.governance_realm_pda,
    )
    .await
    .is_err());

    // Try & remove 0 tokens should fail
    assert!(test_instructions::remove_liquid_stake(
        &test_setup.program_test_ctx,
        alice,
        &test_setup.payer_keypair,
        RemoveLiquidStakeParams { amount: 0 },
        &cortex_stake_reward_mint,
        &test_setup.governance_realm_pda,
    )
    .await
    .is_err());

    // Remove the other half of the stake
    test_instructions::remove_liquid_stake(
        &test_setup.program_test_ctx,
        alice,
        &test_setup.payer_keypair,
        RemoveLiquidStakeParams {
            amount: utils::scale(1, Cortex::LM_DECIMALS),
        },
        &cortex_stake_reward_mint,
        &test_setup.governance_realm_pda,
    )
    .await
    .unwrap();
}
