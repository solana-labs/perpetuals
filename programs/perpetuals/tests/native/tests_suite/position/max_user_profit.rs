use {
    crate::{
        instructions,
        utils::{self, fixtures, pda},
    },
    bonfida_test_utils::ProgramTestExt,
    perpetuals::{
        instructions::{ClosePositionParams, OpenPositionParams, SetTestOraclePriceParams},
        state::{custody::PricingParams, position::Side},
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

const KEYPAIRS_COUNT: usize = 8;

const ETH_DECIMALS: u8 = 9;
const USDC_DECIMALS: u8 = 6;

pub async fn max_user_profit() {
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

    // Deploy the perpetuals program onchain as upgradeable program
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

    // Initialize and fund associated token accounts
    {
        let lm_token_mint = utils::pda::get_lm_token_mint_pda().0;

        // Alice: mint 10k ETH, 10000 USDC, init LM account
        {
            utils::initialize_and_fund_token_account(
                &mut program_test_ctx,
                &eth_mint,
                &keypairs[USER_ALICE].pubkey(),
                &keypairs[ROOT_AUTHORITY],
                utils::scale(10_000, ETH_DECIMALS),
            )
            .await;

            utils::initialize_and_fund_token_account(
                &mut program_test_ctx,
                &usdc_mint,
                &keypairs[USER_ALICE].pubkey(),
                &keypairs[ROOT_AUTHORITY],
                utils::scale(1_000, USDC_DECIMALS),
            )
            .await;
            utils::initialize_token_account(
                &mut program_test_ctx,
                &lm_token_mint,
                &keypairs[USER_ALICE].pubkey(),
            )
            .await;
        }

        // Martin: mint 2 ETH, 1000USDC, init LM account
        {
            utils::initialize_and_fund_token_account(
                &mut program_test_ctx,
                &eth_mint,
                &keypairs[USER_MARTIN].pubkey(),
                &keypairs[ROOT_AUTHORITY],
                utils::scale(2, ETH_DECIMALS),
            )
            .await;

            utils::initialize_and_fund_token_account(
                &mut program_test_ctx,
                &usdc_mint,
                &keypairs[USER_MARTIN].pubkey(),
                &keypairs[ROOT_AUTHORITY],
                utils::scale(1_000, USDC_DECIMALS),
            )
            .await;

            utils::initialize_token_account(
                &mut program_test_ctx,
                &lm_token_mint,
                &keypairs[USER_MARTIN].pubkey(),
            )
            .await;
        }
    }

    let (pool_pda, _, _, _, custodies_infos) = utils::setup_pool_with_custodies_and_liquidity(
        &mut program_test_ctx,
        &keypairs[MULTISIG_MEMBER_A],
        "FOO",
        &keypairs[PAYER],
        &cortex_stake_reward_mint,
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
                liquidity_amount: utils::scale(1_000, USDC_DECIMALS),
                payer: utils::copy_keypair(&keypairs[USER_ALICE]),
            },
            utils::SetupCustodyWithLiquidityParams {
                setup_custody_params: utils::SetupCustodyParams {
                    mint: eth_mint,
                    decimals: ETH_DECIMALS,
                    is_stable: false,
                    target_ratio: utils::ratio_from_percentage(100.0),
                    min_ratio: utils::ratio_from_percentage(0.0),
                    max_ratio: utils::ratio_from_percentage(100.0),
                    initial_price: utils::scale(1_500, ETH_DECIMALS),
                    initial_conf: utils::scale(10, ETH_DECIMALS),
                    pricing_params: Some(PricingParams {
                        // Expressed in BPS, with BPS = 10_000
                        // 2_500 = x0.25, 10_000 = x1, 50_000 = x5
                        max_payoff_mult: 2_500,
                        ..fixtures::pricing_params_regular(false)
                    }),
                    permissions: None,
                    fees: None,
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale(10_000, ETH_DECIMALS),
                payer: utils::copy_keypair(&keypairs[USER_ALICE]),
            },
        ],
    )
    .await;

    // Martin: Open 1 ETH long position x5
    let position_pda = instructions::test_open_position(
        &mut program_test_ctx,
        &keypairs[USER_MARTIN],
        &keypairs[PAYER],
        &pool_pda,
        &eth_mint,
        &cortex_stake_reward_mint,
        OpenPositionParams {
            // max price paid (slippage implied)
            price: utils::scale(1_550, ETH_DECIMALS),
            collateral: utils::scale(1, ETH_DECIMALS),
            size: utils::scale(5, ETH_DECIMALS),
            side: Side::Long,
        },
    )
    .await
    .unwrap()
    .0;

    // Makes ETH price to raise 100%
    {
        let eth_test_oracle_pda = custodies_infos[0].test_oracle_pda;
        let eth_custody_pda = custodies_infos[0].custody_pda;

        let publish_time = utils::get_current_unix_timestamp(&mut program_test_ctx).await;

        instructions::test_set_test_oracle_price(
            &mut program_test_ctx,
            &keypairs[MULTISIG_MEMBER_A],
            &keypairs[PAYER],
            &pool_pda,
            &eth_custody_pda,
            &eth_test_oracle_pda,
            SetTestOraclePriceParams {
                price: utils::scale(3_000, ETH_DECIMALS),
                expo: -(ETH_DECIMALS as i32),
                conf: utils::scale(10, ETH_DECIMALS),
                publish_time,
            },
            multisig_signers,
        )
        .await
        .unwrap();
    }

    instructions::test_close_position(
        &mut program_test_ctx,
        &keypairs[USER_MARTIN],
        &keypairs[PAYER],
        &pool_pda,
        &eth_mint,
        &cortex_stake_reward_mint,
        &position_pda,
        ClosePositionParams {
            // lowest exit price paid (slippage implied)
            price: utils::scale(2_940, 6),
        },
    )
    .await
    .unwrap();

    // Check user gains
    {
        let martin_eth_pda =
            utils::find_associated_token_account(&keypairs[USER_MARTIN].pubkey(), &eth_mint).0;

        let martin_eth_balance =
            utils::get_token_account_balance(&mut program_test_ctx, martin_eth_pda).await;

        // Gains are limited to 0.25 * 5 = 1.25 ETH
        // True gains should be 2.5 ETH less fees (price did x2 on x5 leverage)
        assert_eq!(martin_eth_balance, utils::scale_f64(2.7, ETH_DECIMALS));
    }
}
