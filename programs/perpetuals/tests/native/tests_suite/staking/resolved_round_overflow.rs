use {
    crate::{
        test_instructions,
        utils::{self, pda},
    },
    maplit::hashmap,
    perpetuals::{
        instructions::{AddLiquidStakeParams, AddLiquidityParams, AddVestParams},
        state::{
            cortex::Cortex,
            staking::{Staking, StakingRound},
        },
    },
};

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;

pub async fn resolved_round_overflow() {
    let test_setup = utils::TestSetup::new(
        vec![utils::UserParam {
            name: "alice",
            token_balances: hashmap! {
                "usdc" => utils::scale(3_000, USDC_DECIMALS),
                "eth" => utils::scale(50, ETH_DECIMALS),
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

    let eth_mint = test_setup.get_mint_by_name("eth");

    let admin_a = test_setup.get_multisig_member_keypair_by_name("admin_a");

    let multisig_signers = test_setup.get_multisig_signers();

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
                amount: utils::scale(10, Cortex::LM_DECIMALS),
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

    let stakes_claim_cron_thread_id =
        utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await as u64;

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

    // Alice: add liquid staking
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

    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
    let staking_pda = pda::get_staking_pda(&lm_token_mint_pda).0;

    // Check initial state of resolved rounds
    {
        let staking =
            utils::get_account::<Staking>(&test_setup.program_test_ctx, staking_pda).await;

        assert_eq!(staking.resolved_staking_rounds.len(), 0);
        assert_eq!(staking.resolved_reward_token_amount, 0);
        assert_eq!(staking.resolved_staked_token_amount, 0);
    }

    //
    // During the test, never trigger auto-claim cron to simulate claim default
    //

    // Fill resolved rounds to the max
    for _ in 0..(StakingRound::MAX_RESOLVED_ROUNDS + 1) {
        // Use add liquidity to generate rewards for the current round to be able to differentiate rounds
        {
            // Generate platform activity to fill current round' rewards
            test_instructions::add_liquidity(
                &test_setup.program_test_ctx,
                alice,
                &test_setup.payer_keypair,
                &test_setup.pool_pda,
                &eth_mint,
                AddLiquidityParams {
                    amount_in: utils::scale_f64(0.01, ETH_DECIMALS),
                    min_lp_amount_out: 1,
                },
            )
            .await
            .unwrap();
        }

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
        }
    }

    // Add one more round and check it doesn't overflow
    {
        // Use add liquidity to generate rewards for the current round to be able to differentiate rounds
        {
            // Generate platform activity to fill current round' rewards
            test_instructions::add_liquidity(
                &test_setup.program_test_ctx,
                alice,
                &test_setup.payer_keypair,
                &test_setup.pool_pda,
                &eth_mint,
                AddLiquidityParams {
                    amount_in: utils::scale_f64(0.01, ETH_DECIMALS),
                    min_lp_amount_out: 1,
                },
            )
            .await
            .unwrap();
        }

        let staking_before =
            utils::get_account::<Staking>(&test_setup.program_test_ctx, staking_pda).await;

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
        }

        let staking_after =
            utils::get_account::<Staking>(&test_setup.program_test_ctx, staking_pda).await;

        assert_eq!(
            staking_before.resolved_staking_rounds.len(),
            StakingRound::MAX_RESOLVED_ROUNDS,
        );

        assert_eq!(
            staking_after.resolved_staking_rounds.len(),
            StakingRound::MAX_RESOLVED_ROUNDS
        );

        // rounds should be the same except the last one
        for i in 0..(StakingRound::MAX_RESOLVED_ROUNDS - 1) {
            assert_eq!(
                staking_before.resolved_staking_rounds[i + 1],
                staking_after.resolved_staking_rounds[i],
            );
        }

        assert_eq!(
            staking_before.resolved_staking_rounds.len(),
            StakingRound::MAX_RESOLVED_ROUNDS,
        );
    }
}
