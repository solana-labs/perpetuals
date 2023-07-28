use {
    crate::{
        test_instructions,
        utils::{self, pda},
    },
    maplit::hashmap,
    perpetuals::{
        instructions::{AddLiquidStakeParams, AddLockedStakeParams, AddVestParams},
        state::{
            cortex::Cortex,
            staking::{Staking, StakingRound},
        },
    },
    solana_sdk::signer::Signer,
};

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;

pub async fn multiple_stakers_get_correct_rewards() {
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
            utils::UserParam {
                name: "paul",
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
    let paul = test_setup.get_user_keypair_by_name("paul");

    let admin_a = test_setup.get_multisig_member_keypair_by_name("admin_a");

    let cortex_stake_reward_mint = test_setup.get_cortex_stake_reward_mint();
    let multisig_signers = test_setup.get_multisig_signers();

    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;

    // Prep work: alice/martin/paul get 2 governance tokens using vesting
    {
        let current_time = utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await;

        let users = [alice, martin, paul];

        for user in users.into_iter() {
            test_instructions::add_vest(
                &test_setup.program_test_ctx,
                admin_a,
                &test_setup.payer_keypair,
                user,
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
        }

        // Move until vest end
        utils::warp_forward(&test_setup.program_test_ctx, utils::days_in_seconds(7)).await;

        for user in users.into_iter() {
            test_instructions::claim_vest(
                &test_setup.program_test_ctx,
                &test_setup.payer_keypair,
                user,
                &test_setup.governance_realm_pda,
            )
            .await
            .unwrap();
        }
    }

    // Init staking for alice/martin/paul
    {
        let users = [alice, martin, paul];

        let stakes_claim_cron_thread_id =
            utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await as u64;

        for user in users {
            test_instructions::init_user_staking(
                &test_setup.program_test_ctx,
                user,
                &test_setup.payer_keypair,
                &lm_token_mint_pda,
                perpetuals::instructions::InitUserStakingParams {
                    stakes_claim_cron_thread_id,
                },
            )
            .await
            .unwrap();
        }
    }

    utils::warp_forward(&test_setup.program_test_ctx, 1).await;

    // Add staking for alice/martin/paul with different amounts & time
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

        let stake_resolution_thread_id =
            utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await as u64;

        test_instructions::add_locked_stake(
            &test_setup.program_test_ctx,
            martin,
            &test_setup.payer_keypair,
            AddLockedStakeParams {
                amount: utils::scale_f64(1.5, Cortex::LM_DECIMALS),
                locked_days: 30,
                stake_resolution_thread_id,
            },
            &lm_token_mint_pda,
            &test_setup.governance_realm_pda,
        )
        .await
        .unwrap();

        test_instructions::add_locked_stake(
            &test_setup.program_test_ctx,
            paul,
            &test_setup.payer_keypair,
            AddLockedStakeParams {
                amount: utils::scale(1, Cortex::LM_DECIMALS),
                locked_days: 60,
                stake_resolution_thread_id,
            },
            &lm_token_mint_pda,
            &test_setup.governance_realm_pda,
        )
        .await
        .unwrap();
    }

    utils::warp_forward(&test_setup.program_test_ctx, 1).await;

    // warp to the next round and resolve the current one
    // this round bear no rewards for the new staking
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

    // warp to the next round and resolve the current one
    // this round bear ewards for the new staking
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

    // Claims tokens for alice/martin/paul
    // Should get different share

    let alice_staking_reward_token_account_address =
        utils::find_associated_token_account(&alice.pubkey(), &cortex_stake_reward_mint).0;

    let martin_staking_reward_token_account_address =
        utils::find_associated_token_account(&martin.pubkey(), &cortex_stake_reward_mint).0;

    let paul_staking_reward_token_account_address =
        utils::find_associated_token_account(&paul.pubkey(), &cortex_stake_reward_mint).0;

    // Claim when there is one round worth of rewards to claim
    {
        // Claim alice
        {
            let balance_before = utils::get_token_account_balance(
                &test_setup.program_test_ctx,
                alice_staking_reward_token_account_address,
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

            let balance_after = utils::get_token_account_balance(
                &test_setup.program_test_ctx,
                alice_staking_reward_token_account_address,
            )
            .await;

            assert_eq!(balance_after - balance_before, 7_996_958);
        }

        // Claim martin
        {
            let balance_before = utils::get_token_account_balance(
                &test_setup.program_test_ctx,
                martin_staking_reward_token_account_address,
            )
            .await;

            test_instructions::claim_stakes(
                &test_setup.program_test_ctx,
                martin,
                martin,
                &test_setup.payer_keypair,
                &lm_token_mint_pda,
            )
            .await
            .unwrap();

            let balance_after = utils::get_token_account_balance(
                &test_setup.program_test_ctx,
                martin_staking_reward_token_account_address,
            )
            .await;

            assert_eq!(balance_after - balance_before, 14_994_297);
        }

        // Claim paul
        {
            let balance_before = utils::get_token_account_balance(
                &test_setup.program_test_ctx,
                paul_staking_reward_token_account_address,
            )
            .await;

            test_instructions::claim_stakes(
                &test_setup.program_test_ctx,
                paul,
                paul,
                &test_setup.payer_keypair,
                &lm_token_mint_pda,
            )
            .await
            .unwrap();

            let balance_after = utils::get_token_account_balance(
                &test_setup.program_test_ctx,
                paul_staking_reward_token_account_address,
            )
            .await;

            assert_eq!(balance_after - balance_before, 12_475_255);
        }
    }

    // Assert all rewards got distributed
    {
        let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
        let staking_pda = pda::get_staking_pda(&lm_token_mint_pda).0;

        let staking_account =
            utils::get_account::<Staking>(&test_setup.program_test_ctx, staking_pda).await;

        // Accept dust due to precision loss
        assert!(staking_account.resolved_reward_token_amount <= 100);
    }
}
