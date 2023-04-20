use bonfida_test_utils::ProgramTestContextExt;
use perpetuals::{
    instructions::{ClosePositionParams, OpenPositionParams},
    state::position::Side,
};

use {
    crate::adapters,
    crate::utils::pda,
    crate::{
        instructions,
        utils::{self, fixtures, scale},
    },
    bonfida_test_utils::ProgramTestExt,
    perpetuals::{
        instructions::{AddStakeParams, AddVestParams},
        state::{cortex::Cortex, perpetuals::Perpetuals},
    },
    solana_program_test::ProgramTest,
    solana_sdk::signer::Signer,
};

const ROOT_AUTHORITY: usize = 0;
const PERPETUALS_UPGRADE_AUTHORITY: usize = 1;
const MULTISIG_MEMBER_A: usize = 2;
const MULTISIG_MEMBER_B: usize = 3;
const MULTISIG_MEMBER_C: usize = 4;
const PAYER: usize = 5;
const USER_ALICE: usize = 6;
const USER_MARTIN: usize = 7;

const KEYPAIRS_COUNT: usize = 9;

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;
const LM_TOKEN_DECIMALS: u8 = 6;

// this test is about filling the maximum number of staking rounds the systme can hold (StakingRound::MAX_RESOLVED_ROUNDS)
// and playing around that limit for different edge cases

pub async fn test_staking_rewards() {
    let mut program_test = ProgramTest::default();

    // Initialize the accounts that will be used during the test suite
    let keypairs =
        utils::create_and_fund_multiple_accounts(&mut program_test, KEYPAIRS_COUNT).await;

    // Initialize mints
    let usdc_mint = program_test
        .add_mint(None, USDC_DECIMALS, &keypairs[ROOT_AUTHORITY].pubkey())
        .0;
    let eth_mint = program_test
        .add_mint(None, ETH_DECIMALS, &keypairs[ROOT_AUTHORITY].pubkey())
        .0;

    // Deploy programs
    utils::add_perpetuals_program(&mut program_test, &keypairs[PERPETUALS_UPGRADE_AUTHORITY]).await;
    utils::add_spl_governance_program(&mut program_test, &keypairs[PERPETUALS_UPGRADE_AUTHORITY])
        .await;

    // Start the client and connect to localnet validator
    let mut program_test_ctx = program_test.start_with_context().await;

    let upgrade_authority = &keypairs[PERPETUALS_UPGRADE_AUTHORITY];

    let multisig_signers = &[
        &keypairs[MULTISIG_MEMBER_A],
        &keypairs[MULTISIG_MEMBER_B],
        &keypairs[MULTISIG_MEMBER_C],
    ];

    let governance_realm_pda = pda::get_governance_realm_pda("ADRENA".to_string());

    // mint for the payouts of the LM token staking (ADX staking)
    let cortex_stake_reward_mint = usdc_mint;

    instructions::test_init(
        &mut program_test_ctx,
        upgrade_authority,
        fixtures::init_params_permissions_full(1),
        &governance_realm_pda,
        &cortex_stake_reward_mint,
        multisig_signers,
    )
    .await
    .unwrap();

    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;

    adapters::spl_governance::create_realm(
        &mut program_test_ctx,
        &keypairs[ROOT_AUTHORITY],
        &keypairs[PAYER],
        "ADRENA".to_string(),
        utils::scale(10_000, LM_TOKEN_DECIMALS),
        &lm_token_mint_pda,
    )
    .await
    .unwrap();

    // Initialize and fund associated token accounts
    {
        let lm_token_mint = utils::pda::get_lm_token_mint_pda().0;

        // Alice: mint 1k USDC, mint 2 ETH,  create LM token account, create stake reward token account
        {
            utils::initialize_and_fund_token_account(
                &mut program_test_ctx,
                &usdc_mint,
                &keypairs[USER_ALICE].pubkey(),
                &keypairs[ROOT_AUTHORITY],
                utils::scale(1_000, USDC_DECIMALS),
            )
            .await;

            utils::initialize_and_fund_token_account(
                &mut program_test_ctx,
                &eth_mint,
                &keypairs[USER_ALICE].pubkey(),
                &keypairs[ROOT_AUTHORITY],
                utils::scale(2, ETH_DECIMALS),
            )
            .await;

            utils::initialize_token_account(
                &mut program_test_ctx,
                &lm_token_mint,
                &keypairs[USER_ALICE].pubkey(),
            )
            .await;

            utils::initialize_token_account(
                &mut program_test_ctx,
                &cortex_stake_reward_mint,
                &keypairs[USER_ALICE].pubkey(),
            )
            .await;
        }

        // Martin: mint 1k USDC, mint 2 ETH,  create LM token account, create stake reward token account
        {
            utils::initialize_and_fund_token_account(
                &mut program_test_ctx,
                &usdc_mint,
                &keypairs[USER_MARTIN].pubkey(),
                &keypairs[ROOT_AUTHORITY],
                utils::scale(1_000, USDC_DECIMALS),
            )
            .await;

            utils::initialize_and_fund_token_account(
                &mut program_test_ctx,
                &eth_mint,
                &keypairs[USER_MARTIN].pubkey(),
                &keypairs[ROOT_AUTHORITY],
                utils::scale(2, ETH_DECIMALS),
            )
            .await;

            utils::initialize_token_account(
                &mut program_test_ctx,
                &lm_token_mint,
                &keypairs[USER_MARTIN].pubkey(),
            )
            .await;

            utils::initialize_token_account(
                &mut program_test_ctx,
                &cortex_stake_reward_mint,
                &keypairs[USER_MARTIN].pubkey(),
            )
            .await;
        }
    }

    let (pool_pda, _, _, _, _) = utils::setup_pool_with_custodies_and_liquidity(
        &mut program_test_ctx,
        &keypairs[MULTISIG_MEMBER_A],
        "FOO",
        &keypairs[PAYER],
        multisig_signers,
        vec![
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
                    mint: usdc_mint,
                    decimals: USDC_DECIMALS,
                    is_stable: true,
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
                // Alice: add 1k USDC liquidity
                liquidity_amount: utils::scale(1_000, USDC_DECIMALS),
                payer: utils::copy_keypair(&keypairs[USER_ALICE]),
            },
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
                    mint: eth_mint,
                    decimals: ETH_DECIMALS,
                    is_stable: false,
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
                // Martin: add 1 ETH liquidity
                liquidity_amount: utils::scale(1, ETH_DECIMALS),
                payer: utils::copy_keypair(&keypairs[USER_MARTIN]),
            },
        ],
    )
    .await;

    // Prep work: Vest and claim (to get some governance tokens)
    {
        // Alice: vest 2 token, unlockable at 50% unlock share (circulating supply 2 tokens)
        instructions::test_add_vest(
            &mut program_test_ctx,
            &keypairs[MULTISIG_MEMBER_A],
            &keypairs[PAYER],
            &keypairs[USER_ALICE],
            &governance_realm_pda,
            &AddVestParams {
                amount: utils::scale(2, Cortex::LM_DECIMALS),
                unlock_share: utils::scale_f64(0.51, Perpetuals::BPS_DECIMALS),
            },
            multisig_signers,
        )
        .await
        .unwrap();

        // // Martin: vest 2 token, unlockable at 50% unlock share (circulating supply 4 tokens)
        // instructions::test_add_vest(
        //     &mut program_test_ctx,
        //     &keypairs[MULTISIG_MEMBER_A],
        //     &keypairs[PAYER],
        //     &keypairs[USER_MARTIN],
        //     &governance_realm_pda,
        //     &AddVestParams {
        //         amount: utils::scale(2, Cortex::LM_DECIMALS),
        //         unlock_share: utils::scale_f64(0.99, Perpetuals::BPS_DECIMALS),
        //     },
        //     multisig_signers,
        // )
        // .await
        // .unwrap();

        // Alice: claim vest
        instructions::test_claim_vest(
            &mut program_test_ctx,
            &keypairs[PAYER],
            &keypairs[USER_ALICE],
            &governance_realm_pda,
        )
        .await
        .unwrap();

        // // Martin: claim vest
        // instructions::test_claim_vest(
        //     &mut program_test_ctx,
        //     &keypairs[PAYER],
        //     &keypairs[USER_MARTIN],
        //     &governance_realm_pda,
        // )
        // .await
        // .unwrap();
    }

    // Prep work: Generate some platform activity to fill current round' rewards
    {
        // Martin: Open 0.1 ETH position
        let position_pda = instructions::test_open_position(
            &mut program_test_ctx,
            &keypairs[USER_MARTIN],
            &keypairs[PAYER],
            &pool_pda,
            &eth_mint,
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
        instructions::test_close_position(
            &mut program_test_ctx,
            &keypairs[USER_MARTIN],
            &keypairs[PAYER],
            &pool_pda,
            &eth_mint,
            &position_pda,
            ClosePositionParams {
                // lowest exit price paid (slippage implied)
                price: utils::scale(1_450, USDC_DECIMALS),
            },
        )
        .await
        .unwrap();
    }

    // happy path: stake, resolve, claim
    {
        // GIVEN
        let alice_stake_reward_token_account_address = utils::find_associated_token_account(
            &keypairs[USER_ALICE].pubkey(),
            &cortex_stake_reward_mint,
        )
        .0;
        let alice_stake_reward_token_account_before = program_test_ctx
            .get_token_account(alice_stake_reward_token_account_address)
            .await
            .unwrap();

        // WHEN
        // Alice: add stake LM token
        instructions::test_add_stake(
            &mut program_test_ctx,
            &keypairs[USER_ALICE],
            &keypairs[PAYER],
            AddStakeParams {
                amount: scale(1, Cortex::LM_DECIMALS),
            },
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        // go to next round warps 6h in the future
        utils::warp_forward(&mut program_test_ctx, 3_600 * 6).await;

        // resolve round
        instructions::test_resolve_staking_round(
            &mut program_test_ctx,
            &keypairs[USER_ALICE],
            &keypairs[USER_ALICE],
            &keypairs[PAYER],
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        // Alice: test claim stake (no stake account, none)
        instructions::test_claim_stake(
            &mut program_test_ctx,
            &keypairs[USER_ALICE],
            &keypairs[USER_ALICE],
            &keypairs[PAYER],
            &cortex_stake_reward_mint,
        )
        .await
        .unwrap();

        // THEN
        let alice_stake_reward_token_account_after = program_test_ctx
            .get_token_account(alice_stake_reward_token_account_address)
            .await
            .unwrap();

        // alice received stake rewards
        println!(
            "alice_stake_reward_token_account_after.amount {}",
            alice_stake_reward_token_account_after.amount
        );
        println!(
            "alice_stake_reward_token_account_before.amount {}",
            alice_stake_reward_token_account_before.amount
        );
        assert!(
            alice_stake_reward_token_account_after.amount
                > alice_stake_reward_token_account_before.amount
        );
    }
}
