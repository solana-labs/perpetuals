use {
    crate::{
        test_instructions,
        utils::{self, pda},
    },
    maplit::hashmap,
    perpetuals::{
        instructions::{
            AddLiquidStakeParams, AddVestParams, ClosePositionParams, OpenPositionParams,
            RemoveLiquidStakeParams, RemoveLiquidityParams, SwapParams,
        },
        state::{cortex::Cortex, position::Side, staking::StakingRound},
    },
    solana_sdk::signer::Signer,
};

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;

pub async fn basic_interactions() {
    let test_setup = utils::TestSetup::new(
        vec![
            utils::UserParam {
                name: "alice",
                token_balances: hashmap! {
                    "usdc" => utils::scale(1_000, USDC_DECIMALS),
                },
            },
            utils::UserParam {
                name: "martin",
                token_balances: hashmap! {
                    "usdc"  => utils::scale(100, USDC_DECIMALS),
                    "eth"  => utils::scale(2, ETH_DECIMALS),
                },
            },
            utils::UserParam {
                name: "paul",
                token_balances: hashmap! {
                    "usdc"  => utils::scale(150, USDC_DECIMALS),
                    "eth"  => utils::scale(1, ETH_DECIMALS),
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
                liquidity_amount: utils::scale(1_000, USDC_DECIMALS),
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
                payer_user_name: "martin",
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

    let usdc_mint = &test_setup.get_mint_by_name("usdc");
    let eth_mint = &test_setup.get_mint_by_name("eth");

    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
    utils::warp_forward(&test_setup.program_test_ctx, 1).await;

    // Simple open/close position
    {
        // Martin: Open 0.1 ETH position
        let position_pda = test_instructions::open_position(
            &test_setup.program_test_ctx,
            martin,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            eth_mint,
            OpenPositionParams {
                // max price paid (slippage implied)
                price: utils::scale(1_550, USDC_DECIMALS),
                collateral: utils::scale_f64(0.1, ETH_DECIMALS),
                size: utils::scale_f64(0.1, ETH_DECIMALS),
                side: Side::Long,
            },
        )
        .await
        .unwrap()
        .0;

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
                price: utils::scale(1_450, USDC_DECIMALS),
            },
        )
        .await
        .unwrap();
    }

    // Simple swaps
    {
        let paul_eth_ata = utils::find_associated_token_account(&paul.pubkey(), eth_mint).0;
        let paul_usdc_ata = utils::find_associated_token_account(&paul.pubkey(), usdc_mint).0;

        // Paul: Swap 150 USDC for ETH
        {
            let eth_balance_before =
                utils::get_token_account_balance(&test_setup.program_test_ctx, paul_eth_ata).await;

            let usdc_balance_before =
                utils::get_token_account_balance(&test_setup.program_test_ctx, paul_usdc_ata).await;

            test_instructions::swap(
                &test_setup.program_test_ctx,
                paul,
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

            let eth_balance_after =
                utils::get_token_account_balance(&test_setup.program_test_ctx, paul_eth_ata).await;

            let usdc_balance_after =
                utils::get_token_account_balance(&test_setup.program_test_ctx, paul_usdc_ata).await;

            assert_eq!(eth_balance_after - eth_balance_before, 96_272_504);
            assert_eq!(usdc_balance_before - usdc_balance_after, 150_000_000);
        }

        // Paul: Swap 0.1 ETH for 150 USDC
        {
            let eth_balance_before =
                utils::get_token_account_balance(&test_setup.program_test_ctx, paul_eth_ata).await;

            let usdc_balance_before =
                utils::get_token_account_balance(&test_setup.program_test_ctx, paul_usdc_ata).await;

            test_instructions::swap(
                &test_setup.program_test_ctx,
                paul,
                &test_setup.payer_keypair,
                &test_setup.pool_pda,
                usdc_mint,
                // The program receives ETH
                eth_mint,
                SwapParams {
                    amount_in: utils::scale_f64(0.1, ETH_DECIMALS),
                    min_amount_out: utils::scale(140, USDC_DECIMALS),
                },
            )
            .await
            .unwrap();

            let eth_balance_after =
                utils::get_token_account_balance(&test_setup.program_test_ctx, paul_eth_ata).await;

            let usdc_balance_after =
                utils::get_token_account_balance(&test_setup.program_test_ctx, paul_usdc_ata).await;

            assert_eq!(eth_balance_before - eth_balance_after, 100_000_000);
            assert_eq!(usdc_balance_after - usdc_balance_before, 143_579_400);
        }
    }

    // Remove liquidity
    {
        let alice_lp_token =
            utils::find_associated_token_account(&alice.pubkey(), &test_setup.lp_token_mint_pda).0;

        let alice_lp_token_balance =
            utils::get_token_account_balance(&test_setup.program_test_ctx, alice_lp_token).await;

        // Alice: Remove 100% of provided liquidity (1k USDC less fees)
        test_instructions::remove_liquidity(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            usdc_mint,
            RemoveLiquidityParams {
                lp_amount_in: alice_lp_token_balance,
                min_amount_out: 1,
            },
        )
        .await
        .unwrap();
    }

    // Simple vest and claim
    {
        let current_time = utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await;

        // Alice: vest 2 token, unlock period from now to in 7 days
        test_instructions::add_vest(
            &test_setup.program_test_ctx,
            admin_a,
            &test_setup.payer_keypair,
            alice,
            &test_setup.governance_realm_pda,
            &AddVestParams {
                amount: utils::scale(2, Cortex::LM_DECIMALS),
                unlock_start_timestamp: current_time,
                unlock_end_timestamp: utils::days_in_seconds(7) + current_time,
            },
            &multisig_signers,
        )
        .await
        .unwrap();

        // warp to have tokens to claim
        utils::warp_forward(&test_setup.program_test_ctx, utils::days_in_seconds(7)).await;

        // Alice: claim vest
        test_instructions::claim_vest(
            &test_setup.program_test_ctx,
            &test_setup.payer_keypair,
            alice,
            &test_setup.governance_realm_pda,
        )
        .await
        .unwrap();
    }

    // UserStaking
    {
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

        // Alice: add liquid stake
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

        // Alice: claim stake (nothing to be claimed yet)
        test_instructions::claim_stakes(
            &test_setup.program_test_ctx,
            alice,
            alice,
            &test_setup.payer_keypair,
            &lm_token_mint_pda,
        )
        .await
        .unwrap();

        // Alice: remove liquid staking
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

        // warps to the next round
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
