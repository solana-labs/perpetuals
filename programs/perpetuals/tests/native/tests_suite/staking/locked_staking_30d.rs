use {
    crate::{instructions, utils},
    maplit::hashmap,
    perpetuals::{
        instructions::{
            AddLiquidityParams, AddLockedStakeParams, AddVestParams, RemoveLockedStakeParams,
        },
        state::cortex::{Cortex, StakingRound},
    },
    solana_sdk::signer::Signer,
};

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;

pub async fn locked_staking_30d() {
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
    )
    .await;

    let alice = test_setup.get_user_keypair_by_name("alice");
    let martin = test_setup.get_user_keypair_by_name("martin");

    let admin_a = test_setup.get_multisig_member_keypair_by_name("admin_a");

    let cortex_stake_reward_mint = test_setup.get_cortex_stake_reward_mint();
    let multisig_signers = test_setup.get_multisig_signers();

    let eth_mint = &test_setup.get_mint_by_name("eth");

    let clockwork_worker = test_setup.get_clockwork_worker();

    // Prep work: Alice get 2 governance tokens using vesting
    {
        let current_time =
            utils::get_current_unix_timestamp(&mut test_setup.program_test_ctx.borrow_mut()).await;

        instructions::test_add_vest(
            &mut test_setup.program_test_ctx.borrow_mut(),
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
        utils::warp_forward(
            &mut test_setup.program_test_ctx.borrow_mut(),
            utils::days_in_seconds(7),
        )
        .await;

        instructions::test_claim_vest(
            &mut test_setup.program_test_ctx.borrow_mut(),
            &test_setup.payer_keypair,
            alice,
            &test_setup.governance_realm_pda,
        )
        .await
        .unwrap();
    }

    let alice_stake_reward_token_account_address =
        utils::find_associated_token_account(&alice.pubkey(), &cortex_stake_reward_mint).0;

    // Alice: start 30d locked staking
    {
        let stakes_claim_cron_thread_id =
            utils::get_current_unix_timestamp(&mut test_setup.program_test_ctx.borrow_mut()).await
                as u64;

        instructions::test_init_staking(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            &test_setup.payer_keypair,
            &cortex_stake_reward_mint,
            perpetuals::instructions::InitStakingParams {
                stakes_claim_cron_thread_id,
            },
        )
        .await
        .unwrap();

        utils::warp_forward(&mut test_setup.program_test_ctx.borrow_mut(), 1).await;

        let stake_resolution_thread_id =
            utils::get_current_unix_timestamp(&mut test_setup.program_test_ctx.borrow_mut()).await
                as u64;

        instructions::test_add_locked_stake(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            &test_setup.payer_keypair,
            AddLockedStakeParams {
                amount: utils::scale(1, Cortex::LM_DECIMALS),
                locked_days: 30,
                stake_resolution_thread_id,
            },
            &cortex_stake_reward_mint,
            &test_setup.governance_realm_pda,
        )
        .await
        .unwrap();
    }

    utils::warp_forward(&mut test_setup.program_test_ctx.borrow_mut(), 1).await;

    // Alice: claim when there is nothing to claim yet
    {
        let balance_before = utils::get_token_account_balance(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice_stake_reward_token_account_address,
        )
        .await;

        instructions::test_claim_stakes(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test_setup.payer_keypair,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        let balance_after = utils::get_token_account_balance(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice_stake_reward_token_account_address,
        )
        .await;

        assert_eq!(balance_before, balance_after);
    }

    // warp to the next round and resolve the current one
    // this round bear no rewards for the new staking at the staking started during the round
    {
        utils::warp_forward(
            &mut test_setup.program_test_ctx.borrow_mut(),
            StakingRound::ROUND_MIN_DURATION_SECONDS,
        )
        .await;

        instructions::test_resolve_staking_round(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test_setup.payer_keypair,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();
    }

    // Use add liquidity to generate rewards for the current round
    {
        // Generate platform activity to fill current round' rewards
        instructions::test_add_liquidity(
            &mut test_setup.program_test_ctx.borrow_mut(),
            martin,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            eth_mint,
            &cortex_stake_reward_mint,
            AddLiquidityParams {
                amount_in: utils::scale_f64(0.25, ETH_DECIMALS),
                min_lp_amount_out: 1,
            },
        )
        .await
        .unwrap();
    }

    // warp to the next round and resolve the current one
    // this round bear rewards for the new staking at the staking started before the round
    {
        utils::warp_forward(
            &mut test_setup.program_test_ctx.borrow_mut(),
            StakingRound::ROUND_MIN_DURATION_SECONDS,
        )
        .await;

        instructions::test_resolve_staking_round(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test_setup.payer_keypair,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();
    }

    utils::warp_forward(&mut test_setup.program_test_ctx.borrow_mut(), 1).await;

    // Claim when there is one round worth of rewards to claim
    {
        let balance_before = utils::get_token_account_balance(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice_stake_reward_token_account_address,
        )
        .await;

        instructions::test_claim_stakes(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test_setup.payer_keypair,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        let balance_after = utils::get_token_account_balance(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice_stake_reward_token_account_address,
        )
        .await;

        assert_eq!(balance_after - balance_before, 90_094_938);
    }

    // Move 30d in the future where staking have ended
    // TODO: For now we don't have clockwork working, so just do one 30 days round
    {
        utils::warp_forward(
            &mut test_setup.program_test_ctx.borrow_mut(),
            utils::days_in_seconds(30),
        )
        .await;

        instructions::test_resolve_staking_round(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test_setup.payer_keypair,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        /*
        {
            let nb_round = (utils::days_in_seconds(30) as f64
                / StakingRound::ROUND_MIN_DURATION_SECONDS as f64)
                .ceil()
                .to_u64()
                .unwrap();

            for _ in 0..nb_round {
                utils::warp_forward(
                    &mut test_setup.program_test_ctx.borrow_mut(),
                    StakingRound::ROUND_MIN_DURATION_SECONDS,
                )
                .await;

                instructions::test_resolve_staking_round(
                    &mut test_setup.program_test_ctx.borrow_mut(),
                    alice,
                    alice,
                    &test_setup.payer_keypair,
                    &cortex_stake_reward_mint,
                )
                .await
                .unwrap();
            }
        }
        */
    }

    // Remove the stake without resolving it first should fail
    assert!(instructions::test_remove_locked_stake(
        &mut test_setup.program_test_ctx.borrow_mut(),
        alice,
        &test_setup.payer_keypair,
        RemoveLockedStakeParams {
            locked_stake_index: 0,
        },
        &cortex_stake_reward_mint,
        &test_setup.governance_realm_pda,
    )
    .await
    .is_err());

    // Trigger clockwork thread execution manually
    utils::execute_resolve_locked_stake_thread(
        &mut test_setup.program_test_ctx.borrow_mut(),
        &clockwork_worker,
        &test_setup.clockwork_signatory,
        alice,
        &test_setup.payer_keypair,
        &test_setup.governance_realm_pda,
        0,
    )
    .await
    .unwrap();

    /*
    // Resolve the locked stake
    instructions::test_resolve_locked_stakes(
        &mut test_setup.program_test_ctx.borrow_mut(),
        alice,
        alice,
        &test_setup.payer_keypair,
        &test_setup.governance_realm_pda,
    )
    .await
    .unwrap();
    */

    utils::warp_forward(&mut test_setup.program_test_ctx.borrow_mut(), 1).await;

    // Remove the stake
    instructions::test_remove_locked_stake(
        &mut test_setup.program_test_ctx.borrow_mut(),
        alice,
        &test_setup.payer_keypair,
        RemoveLockedStakeParams {
            locked_stake_index: 0,
        },
        &cortex_stake_reward_mint,
        &test_setup.governance_realm_pda,
    )
    .await
    .unwrap();
}
