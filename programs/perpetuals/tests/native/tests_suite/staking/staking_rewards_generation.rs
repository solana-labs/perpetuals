use {
    crate::{
        test_instructions,
        utils::{self, pda},
    },
    maplit::hashmap,
    perpetuals::{
        instructions::{
            AddLiquidityParams, AddLockedStakeParams, AddVestParams, ClosePositionParams,
            OpenPositionParams, RemoveLiquidityParams, SwapParams,
        },
        state::{cortex::Cortex, perpetuals::Perpetuals, position::Side, staking::StakingRound},
    },
};

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;

pub async fn staking_rewards_generation() {
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

    let multisig_signers = test_setup.get_multisig_signers();

    let eth_mint = &test_setup.get_mint_by_name("eth");
    let usdc_mint = &test_setup.get_mint_by_name("usdc");

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

    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
    let lm_staking_pda = pda::get_staking_pda(&lm_token_mint_pda).0;
    let lp_staking_pda = pda::get_staking_pda(&test_setup.lp_token_mint_pda).0;
    let lm_staking_reward_token_vault_pda =
        pda::get_staking_reward_token_vault_pda(&lm_staking_pda).0;
    let lp_staking_reward_token_vault_pda =
        pda::get_staking_reward_token_vault_pda(&lp_staking_pda).0;

    // Create locked stake to make instructions to generate fees for LP locked stakers
    {
        let stakes_claim_cron_thread_id =
            utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await as u64;

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

        utils::warp_forward(&test_setup.program_test_ctx, 1).await;

        let stake_resolution_thread_id =
            utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await as u64;

        test_instructions::add_locked_stake(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            AddLockedStakeParams {
                amount: utils::scale(1_500, Cortex::LM_DECIMALS),
                locked_days: 30,
                stake_resolution_thread_id,
            },
            &test_setup.lp_token_mint_pda,
            &test_setup.governance_realm_pda,
        )
        .await
        .unwrap();

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
            &test_setup.lp_token_mint_pda,
        )
        .await
        .unwrap();
    }

    // Check that add liquidity generates rewards
    {
        let lm_staking_reward_token_account_balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lm_staking_reward_token_vault_pda,
        )
        .await;

        let lp_staking_reward_token_account_balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lp_staking_reward_token_vault_pda,
        )
        .await;

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

        utils::warp_forward(&test_setup.program_test_ctx, 1).await;

        let lm_staking_reward_token_account_balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lm_staking_reward_token_vault_pda,
        )
        .await;

        let lp_staking_reward_token_account_balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lp_staking_reward_token_vault_pda,
        )
        .await;

        // Check rewards has been generated
        assert_eq!(
            lm_staking_reward_token_account_balance_after
                - lm_staking_reward_token_account_balance_before,
            2_710_419,
        );

        assert_eq!(
            lp_staking_reward_token_account_balance_after
                - lp_staking_reward_token_account_balance_before,
            3_773_853,
        );
    }

    utils::warp_forward(&test_setup.program_test_ctx, 1).await;

    // Check that open position generates rewards
    let position_pda = {
        let lm_staking_reward_token_account_balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lm_staking_reward_token_vault_pda,
        )
        .await;

        let lp_staking_reward_token_account_balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lp_staking_reward_token_vault_pda,
        )
        .await;

        // Martin: Open 0.1 ETH long position x1
        let position_pda = test_instructions::open_position(
            &test_setup.program_test_ctx,
            martin,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            eth_mint,
            OpenPositionParams {
                // max price paid (slippage implied)
                price: utils::scale(1_550, ETH_DECIMALS),
                collateral: utils::scale_f64(0.1, ETH_DECIMALS),
                size: utils::scale_f64(0.1, ETH_DECIMALS),
                side: Side::Long,
            },
        )
        .await
        .unwrap()
        .0;

        utils::warp_forward(&test_setup.program_test_ctx, 1).await;

        let lm_staking_reward_token_account_balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lm_staking_reward_token_vault_pda,
        )
        .await;

        let lp_staking_reward_token_account_balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lp_staking_reward_token_vault_pda,
        )
        .await;

        // Check rewards has been generated
        assert_eq!(
            lm_staking_reward_token_account_balance_after
                - lm_staking_reward_token_account_balance_before,
            435_408,
        );

        assert_eq!(
            lp_staking_reward_token_account_balance_after
                - lp_staking_reward_token_account_balance_before,
            547_954,
        );

        position_pda
    };

    utils::warp_forward(&test_setup.program_test_ctx, 1).await;

    // Check that close position generates rewards
    {
        let lm_staking_reward_token_account_balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lm_staking_reward_token_vault_pda,
        )
        .await;

        let lp_staking_reward_token_account_balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lp_staking_reward_token_vault_pda,
        )
        .await;

        // Martin: Close the ETH position
        test_instructions::close_position(
            &test_setup.program_test_ctx,
            martin,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            eth_mint,
            &position_pda,
            ClosePositionParams {
                // lowest exit price paid (slippage implied)
                price: utils::scale(1_485, USDC_DECIMALS),
            },
        )
        .await
        .unwrap();

        utils::warp_forward(&test_setup.program_test_ctx, 1).await;

        let lm_staking_reward_token_account_balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lm_staking_reward_token_vault_pda,
        )
        .await;

        let lp_staking_reward_token_account_balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lp_staking_reward_token_vault_pda,
        )
        .await;

        // Check rewards has been generated
        assert_eq!(
            lm_staking_reward_token_account_balance_after
                - lm_staking_reward_token_account_balance_before,
            439_762,
        );

        assert_eq!(
            lp_staking_reward_token_account_balance_after
                - lp_staking_reward_token_account_balance_before,
            553_433,
        );
    }

    utils::warp_forward(&test_setup.program_test_ctx, 1).await;

    // Check that swap generates rewards
    {
        let lm_staking_reward_token_account_balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lm_staking_reward_token_vault_pda,
        )
        .await;

        let lp_staking_reward_token_account_balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lp_staking_reward_token_vault_pda,
        )
        .await;

        // Martin: Swap 150 USDC for ETH
        test_instructions::swap(
            &test_setup.program_test_ctx,
            martin,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            eth_mint,
            // The program receives USDC
            usdc_mint,
            SwapParams {
                amount_in: utils::scale(150, USDC_DECIMALS),

                // 1% slippage
                min_amount_out: utils::scale(150, USDC_DECIMALS)
                    / utils::scale(1_500, ETH_DECIMALS)
                    * 99
                    / 100,
            },
        )
        .await
        .unwrap();

        utils::warp_forward(&test_setup.program_test_ctx, 1).await;

        let lm_staking_reward_token_account_balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lm_staking_reward_token_vault_pda,
        )
        .await;

        let lp_staking_reward_token_account_balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lp_staking_reward_token_vault_pda,
        )
        .await;

        // Check rewards has been generated
        assert_eq!(
            lm_staking_reward_token_account_balance_after
                - lm_staking_reward_token_account_balance_before,
            745_025,
        );

        assert_eq!(
            lp_staking_reward_token_account_balance_after
                - lp_staking_reward_token_account_balance_before,
            937_601,
        );
    }

    utils::warp_forward(&test_setup.program_test_ctx, 1).await;

    // Check that remove liquidity generates rewards
    {
        let lm_staking_reward_token_account_balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lm_staking_reward_token_vault_pda,
        )
        .await;

        let lp_staking_reward_token_account_balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lp_staking_reward_token_vault_pda,
        )
        .await;

        // Generate platform activity to fill current round' rewards
        test_instructions::remove_liquidity(
            &test_setup.program_test_ctx,
            martin,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            eth_mint,
            RemoveLiquidityParams {
                lp_amount_in: utils::scale(1, Perpetuals::LP_DECIMALS),
                min_amount_out: 0,
            },
        )
        .await
        .unwrap();

        utils::warp_forward(&test_setup.program_test_ctx, 1).await;

        let lm_staking_reward_token_account_balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lm_staking_reward_token_vault_pda,
        )
        .await;

        let lp_staking_reward_token_account_balance_after = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            lp_staking_reward_token_vault_pda,
        )
        .await;

        // Check rewards has been generated
        assert_eq!(
            lm_staking_reward_token_account_balance_after
                - lm_staking_reward_token_account_balance_before,
            8_519,
        );

        assert_eq!(
            lp_staking_reward_token_account_balance_after
                - lp_staking_reward_token_account_balance_before,
            10_721,
        );
    }
}