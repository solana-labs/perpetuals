use {
    crate::{instructions, utils},
    bonfida_test_utils::ProgramTestContextExt,
    maplit::hashmap,
    perpetuals::{
        instructions::{AddStakeParams, AddVestParams, ClosePositionParams, OpenPositionParams},
        state::{
            cortex::{Cortex, StakingRound},
            position::Side,
        },
    },
    solana_sdk::signer::Signer,
};

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;

pub async fn test_staking_rewards_from_open_and_close_position() {
    let test_setup = utils::TestSetup::new(
        vec![
            utils::UserParam {
                name: "alice",
                token_balances: hashmap! {
                    "usdc" => utils::scale(1_000, USDC_DECIMALS),
                    "eth" => utils::scale(2, ETH_DECIMALS),
                },
            },
            utils::UserParam {
                name: "martin",
                token_balances: hashmap! {
                    "usdc" => utils::scale(1_000, USDC_DECIMALS),
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
    )
    .await;

    let alice = test_setup.get_user_keypair_by_name("alice");
    let martin = test_setup.get_user_keypair_by_name("martin");

    let admin_a = test_setup.get_multisig_member_keypair_by_name("admin_a");

    let cortex_stake_reward_mint = test_setup.get_cortex_stake_reward_mint();
    let multisig_signers = test_setup.get_multisig_signers();

    let eth_mint = &test_setup.get_mint_by_name("eth");

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

    // Prep work: Generate some platform activity to fill current round' rewards

    // Martin: Open 0.1 ETH position
    let position_pda = instructions::test_open_position(
        &mut test_setup.program_test_ctx.borrow_mut(),
        martin,
        &test_setup.payer_keypair,
        &test_setup.pool_pda,
        &eth_mint,
        &cortex_stake_reward_mint,
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

    let alice_stake_reward_token_account_address =
        utils::find_associated_token_account(&alice.pubkey(), &cortex_stake_reward_mint).0;

    // happy path: stake, resolve, claim (for the open position)
    {
        // GIVEN
        let alice_stake_reward_token_account_before = test_setup
            .program_test_ctx
            .borrow_mut()
            .get_token_account(alice_stake_reward_token_account_address)
            .await
            .unwrap();

        // Alice: add stake LM token
        instructions::test_add_stake(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            &test_setup.payer_keypair,
            AddStakeParams {
                amount: utils::scale(1, Cortex::LM_DECIMALS),
            },
            &cortex_stake_reward_mint,
            &test_setup.governance_realm_pda,
        )
        .await
        .unwrap();

        // Info - at this stage, alice won't be eligible for current round rewards, as she joined after round inception

        // go to next round warps in the future
        utils::warp_forward(
            &mut test_setup.program_test_ctx.borrow_mut(),
            StakingRound::ROUND_MIN_DURATION_SECONDS,
        )
        .await;

        // resolve round
        instructions::test_resolve_staking_round(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test_setup.payer_keypair,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        // Alice: test_setup claim stake (stake account but not eligible for current round, none)
        instructions::test_claim_stake(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test_setup.payer_keypair,
            &test_setup.governance_realm_pda,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        // THEN
        let alice_stake_reward_token_account_after = test_setup
            .program_test_ctx
            .borrow_mut()
            .get_token_account(alice_stake_reward_token_account_address)
            .await
            .unwrap();

        // alice didn't receive stake rewards
        assert_eq!(
            alice_stake_reward_token_account_after.amount,
            alice_stake_reward_token_account_before.amount
        );

        // Info - new round started, forwarding the previous reward since no stake previously
        // Info - this time Alice was subscribed in time and will qualify for rewards

        // go to next round warps in the future
        utils::warp_forward(
            &mut test_setup.program_test_ctx.borrow_mut(),
            StakingRound::ROUND_MIN_DURATION_SECONDS,
        )
        .await;

        // resolve round
        instructions::test_resolve_staking_round(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test_setup.payer_keypair,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        // Alice: test_setup claim stake (stake account eligible for round, some)
        instructions::test_claim_stake(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test_setup.payer_keypair,
            &test_setup.governance_realm_pda,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        // THEN
        let alice_stake_reward_token_account_before = alice_stake_reward_token_account_after;
        let alice_stake_reward_token_account_after = test_setup
            .program_test_ctx
            .borrow_mut()
            .get_token_account(alice_stake_reward_token_account_address)
            .await
            .unwrap();

        // alice received stake rewards
        assert!(
            alice_stake_reward_token_account_after.amount
                > alice_stake_reward_token_account_before.amount
        );
    }

    // now close the position and see if staking rewards accrued
    {
        // Martin: Close the ETH position
        instructions::test_close_position(
            &mut test_setup.program_test_ctx.borrow_mut(),
            martin,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            &eth_mint,
            &cortex_stake_reward_mint,
            &position_pda,
            ClosePositionParams {
                // lowest exit price paid (slippage implied)
                price: utils::scale(1_450, USDC_DECIMALS),
            },
        )
        .await
        .unwrap();
    }

    // happy path: stake, resolve, claim (for the close position)
    {
        // GIVEN
        let alice_stake_reward_token_account_before = test_setup
            .program_test_ctx
            .borrow_mut()
            .get_token_account(alice_stake_reward_token_account_address)
            .await
            .unwrap();

        // Info - at this stage, alice won't be eligible for current round rewards, as she joined after round inception

        // go to next round warps in the future
        utils::warp_forward(
            &mut test_setup.program_test_ctx.borrow_mut(),
            StakingRound::ROUND_MIN_DURATION_SECONDS,
        )
        .await;

        // resolve round
        instructions::test_resolve_staking_round(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test_setup.payer_keypair,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        // Alice: test_setup claim stake (stake account but not eligible for current round, none)
        instructions::test_claim_stake(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test_setup.payer_keypair,
            &test_setup.governance_realm_pda,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        // THEN
        let alice_stake_reward_token_account_after = test_setup
            .program_test_ctx
            .borrow_mut()
            .get_token_account(alice_stake_reward_token_account_address)
            .await
            .unwrap();

        // alice didn't receive stake rewards
        assert_eq!(
            alice_stake_reward_token_account_after.amount,
            alice_stake_reward_token_account_before.amount
        );

        // Info - new round started, forwarding the previous reward since no stake previously
        // Info - this time Alice was subscribed in time and will qualify for rewards

        // go to next round warps in the future
        utils::warp_forward(
            &mut test_setup.program_test_ctx.borrow_mut(),
            StakingRound::ROUND_MIN_DURATION_SECONDS,
        )
        .await;

        // resolve round
        instructions::test_resolve_staking_round(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test_setup.payer_keypair,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        // Alice: test_setup claim stake (stake account eligible for round, some)
        instructions::test_claim_stake(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            alice,
            &test_setup.payer_keypair,
            &test_setup.governance_realm_pda,
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        // THEN
        let alice_stake_reward_token_account_before = alice_stake_reward_token_account_after;
        let alice_stake_reward_token_account_after = test_setup
            .program_test_ctx
            .borrow_mut()
            .get_token_account(alice_stake_reward_token_account_address)
            .await
            .unwrap();

        // alice received stake rewards
        assert!(
            alice_stake_reward_token_account_after.amount
                > alice_stake_reward_token_account_before.amount
        );
    }
}
