use {
    crate::{test_instructions, utils},
    maplit::hashmap,
    perpetuals::{
        instructions::{AddGenesisLiquidityParams, SwapParams},
        state::{cortex::Cortex, staking::StakingRound},
    },
    solana_sdk::signer::Signer,
};

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 6;
const BTC_DECIMALS: u8 = 6;
const SOL_DECIMALS: u8 = 6;

pub async fn genesis() {
    let test_setup = utils::TestSetup::new(
        vec![utils::UserParam {
            name: "alice",
            token_balances: hashmap! {
                "usdc" => utils::scale(200_000, USDC_DECIMALS),
                "eth" => utils::scale(200, ETH_DECIMALS),
                "btc" => utils::scale(50, BTC_DECIMALS),
            },
        }],
        vec![
            utils::MintParam {
                name: "usdc",
                decimals: USDC_DECIMALS,
            },
            utils::MintParam {
                name: "eth",
                decimals: ETH_DECIMALS,
            },
            utils::MintParam {
                name: "btc",
                decimals: BTC_DECIMALS,
            },
            utils::MintParam {
                name: "sol",
                decimals: SOL_DECIMALS,
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
                    target_ratio: utils::ratio_from_percentage(40.0),
                    min_ratio: utils::ratio_from_percentage(0.0),
                    max_ratio: utils::ratio_from_percentage(100.0),
                    initial_price: utils::scale(1, USDC_DECIMALS),
                    initial_conf: utils::scale_f64(0.01, USDC_DECIMALS),
                    pricing_params: None,
                    permissions: None,
                    fees: None,
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale(0, USDC_DECIMALS),
                payer_user_name: "alice",
            },
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
                    mint_name: "eth",
                    is_stable: false,
                    is_virtual: false,
                    target_ratio: utils::ratio_from_percentage(15.0),
                    min_ratio: utils::ratio_from_percentage(0.0),
                    max_ratio: utils::ratio_from_percentage(100.0),
                    initial_price: utils::scale(1_500, ETH_DECIMALS),
                    initial_conf: utils::scale(10, ETH_DECIMALS),
                    pricing_params: None,
                    permissions: None,
                    fees: None,
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale(0, ETH_DECIMALS),
                payer_user_name: "alice",
            },
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
                    mint_name: "btc",
                    is_stable: false,
                    is_virtual: false,
                    target_ratio: utils::ratio_from_percentage(15.0),
                    min_ratio: utils::ratio_from_percentage(0.0),
                    max_ratio: utils::ratio_from_percentage(100.0),
                    initial_price: utils::scale(30_000, BTC_DECIMALS),
                    initial_conf: utils::scale(10, BTC_DECIMALS),
                    pricing_params: None,
                    permissions: None,
                    fees: None,
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale(0, BTC_DECIMALS),
                payer_user_name: "alice",
            },
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
                    mint_name: "sol",
                    is_stable: false,
                    is_virtual: false,
                    target_ratio: utils::ratio_from_percentage(30.0),
                    min_ratio: utils::ratio_from_percentage(0.0),
                    max_ratio: utils::ratio_from_percentage(100.0),
                    initial_price: utils::scale(25, SOL_DECIMALS),
                    initial_conf: utils::scale_f64(0.2, SOL_DECIMALS),
                    pricing_params: None,
                    permissions: None,
                    fees: None,
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale(0, SOL_DECIMALS),
                payer_user_name: "alice",
            },
        ],
        utils::scale(100_000, Cortex::LM_DECIMALS),
        utils::scale(200_000, Cortex::LM_DECIMALS),
        utils::scale(300_000, Cortex::LM_DECIMALS),
        utils::scale(500_000, Cortex::LM_DECIMALS),
    )
    .await;

    let alice = test_setup.get_user_keypair_by_name("alice");

    let cortex_stake_reward_mint = test_setup.get_cortex_stake_reward_mint();

    let usdc_mint = &test_setup.get_mint_by_name("usdc");
    let eth_mint = &test_setup.get_mint_by_name("eth");
    let btc_mint = &test_setup.get_mint_by_name("btc");

    // Init lp staking
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
    }

    // Add genesis ALP up to $100_000
    {
        let mut lp_stake_resolution_thread_id =
            utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await as u64;

        test_instructions::add_genesis_liquidity(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            usdc_mint,
            &test_setup.governance_realm_pda,
            AddGenesisLiquidityParams {
                amount_in: utils::scale(99_000, USDC_DECIMALS),
                min_lp_amount_out: 1,
                lp_stake_resolution_thread_id,
            },
        )
        .await
        .unwrap();

        utils::warp_forward(&test_setup.program_test_ctx, 1).await;

        lp_stake_resolution_thread_id =
            utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await as u64;

        test_instructions::add_genesis_liquidity(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            eth_mint,
            &test_setup.governance_realm_pda,
            AddGenesisLiquidityParams {
                amount_in: utils::scale(66, ETH_DECIMALS),
                min_lp_amount_out: 1,
                lp_stake_resolution_thread_id,
            },
        )
        .await
        .unwrap();

        utils::warp_forward(&test_setup.program_test_ctx, 1).await;
        lp_stake_resolution_thread_id =
            utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await as u64;

        test_instructions::add_genesis_liquidity(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            btc_mint,
            &test_setup.governance_realm_pda,
            AddGenesisLiquidityParams {
                amount_in: utils::scale_f64(3.3, BTC_DECIMALS),
                min_lp_amount_out: 1,
                lp_stake_resolution_thread_id,
            },
        )
        .await
        .unwrap();

        utils::warp_forward(&test_setup.program_test_ctx, 1).await;
    }

    // Add genesis ALP for more than $100_000 should fail
    {
        let mut lp_stake_resolution_thread_id =
            utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await as u64;

        assert!(test_instructions::add_genesis_liquidity(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            usdc_mint,
            &test_setup.governance_realm_pda,
            AddGenesisLiquidityParams {
                amount_in: utils::scale(10_000, USDC_DECIMALS),
                min_lp_amount_out: 1,
                lp_stake_resolution_thread_id,
            },
        )
        .await
        .is_err());

        utils::warp_forward(&test_setup.program_test_ctx, 1).await;
        lp_stake_resolution_thread_id =
            utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await as u64;

        assert!(test_instructions::add_genesis_liquidity(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            eth_mint,
            &test_setup.governance_realm_pda,
            AddGenesisLiquidityParams {
                amount_in: utils::scale(1, ETH_DECIMALS),
                min_lp_amount_out: 1,
                lp_stake_resolution_thread_id,
            },
        )
        .await
        .is_err());

        utils::warp_forward(&test_setup.program_test_ctx, 1).await;
        lp_stake_resolution_thread_id =
            utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await as u64;

        assert!(test_instructions::add_genesis_liquidity(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            btc_mint,
            &test_setup.governance_realm_pda,
            AddGenesisLiquidityParams {
                amount_in: utils::scale(1, BTC_DECIMALS),
                min_lp_amount_out: 1,
                lp_stake_resolution_thread_id,
            },
        )
        .await
        .is_err());

        utils::warp_forward(&test_setup.program_test_ctx, 1).await;
    }

    {
        // warp to the next round and resolve the current one
        // this round bear no rewards for the new staking as the staking started during the round
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
                &test_setup.lp_token_mint_pda,
            )
            .await
            .unwrap();
        }
    }

    {
        // warp to the next round and resolve the current one
        // this round bear no rewards
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
                &test_setup.lp_token_mint_pda,
            )
            .await
            .unwrap();
        }
    }

    // Do a swap to generate fees to claim
    {
        test_instructions::swap(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            eth_mint,
            // The program receives USDC
            usdc_mint,
            SwapParams {
                amount_in: utils::scale(150, USDC_DECIMALS),
                min_amount_out: utils::scale_f64(0.09, ETH_DECIMALS),
            },
        )
        .await
        .unwrap();
    }

    {
        // warp to the next round and resolve the current one
        // this round bear rewards
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
                &test_setup.lp_token_mint_pda,
            )
            .await
            .unwrap();
        }
    }

    let (alice_lp_staking_reward_token_account_address, _) =
        utils::find_associated_token_account(&alice.pubkey(), &cortex_stake_reward_mint);

    // Claim when there is one round worth of rewards to claim
    {
        let balance_before = utils::get_token_account_balance(
            &test_setup.program_test_ctx,
            alice_lp_staking_reward_token_account_address,
        )
        .await;

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
            alice_lp_staking_reward_token_account_address,
        )
        .await;

        assert_eq!(balance_after - balance_before, 2_073_678);
    }
}
