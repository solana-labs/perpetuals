use {
    crate::{
        instructions,
        utils::{self, fixtures},
    },
    bonfida_test_utils::ProgramTestExt,
    perpetuals::instructions::SetTestOraclePriceParams,
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

const KEYPAIRS_COUNT: usize = 7;

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;

pub async fn lp_token_price() {
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

    // Start the client and connect to localnet validator
    let mut program_test_ctx = program_test.start_with_context().await;

    let upgrade_authority = &keypairs[PERPETUALS_UPGRADE_AUTHORITY];

    let multisig_signers = &[
        &keypairs[MULTISIG_MEMBER_A],
        &keypairs[MULTISIG_MEMBER_B],
        &keypairs[MULTISIG_MEMBER_C],
    ];

    instructions::test_init(
        &mut program_test_ctx,
        upgrade_authority,
        fixtures::init_params_permissions_full(1),
        multisig_signers,
    )
    .await
    .unwrap();

    // Initialize and fund associated token accounts
    {
        // Alice: mint 100k USDC and 50 ETH
        {
            utils::initialize_and_fund_token_account(
                &mut program_test_ctx,
                &usdc_mint,
                &keypairs[USER_ALICE].pubkey(),
                &keypairs[ROOT_AUTHORITY],
                utils::scale(100_000, USDC_DECIMALS),
            )
            .await;

            utils::initialize_and_fund_token_account(
                &mut program_test_ctx,
                &eth_mint,
                &keypairs[USER_ALICE].pubkey(),
                &keypairs[ROOT_AUTHORITY],
                utils::scale(50, ETH_DECIMALS),
            )
            .await;
        }
    }

    // Set the pool with 50%/50% ETH/USDC liquidity
    let (pool_pda, _, lp_token_mint_pda, _, custodies_infos) =
        utils::setup_pool_with_custodies_and_liquidity(
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
                    liquidity_amount: utils::scale(15_000, USDC_DECIMALS),
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
                    liquidity_amount: utils::scale(10, ETH_DECIMALS),
                    payer: utils::copy_keypair(&keypairs[USER_ALICE]),
                },
            ],
        )
        .await;

    // Check LP token price after pool setup
    assert_eq!(
        instructions::test_get_lp_token_price(
            &mut program_test_ctx,
            &keypairs[PAYER],
            &pool_pda,
            &lp_token_mint_pda,
        )
        .await
        .unwrap(),
        1_074_388
    );

    // Increase asset price and check that lp token price increase
    {
        // Makes ETH price to increase of 10%
        {
            let eth_test_oracle_pda = custodies_infos[1].test_oracle_pda;
            let eth_custody_pda = custodies_infos[1].custody_pda;

            let publish_time = utils::get_current_unix_timestamp(&mut program_test_ctx).await;

            instructions::test_set_test_oracle_price(
                &mut program_test_ctx,
                &keypairs[MULTISIG_MEMBER_A],
                &keypairs[PAYER],
                &pool_pda,
                &eth_custody_pda,
                &eth_test_oracle_pda,
                SetTestOraclePriceParams {
                    price: utils::scale(1_650, ETH_DECIMALS),
                    expo: -(ETH_DECIMALS as i32),
                    conf: utils::scale(10, ETH_DECIMALS),
                    publish_time,
                },
                multisig_signers,
            )
            .await
            .unwrap();
        }

        assert_eq!(
            instructions::test_get_lp_token_price(
                &mut program_test_ctx,
                &keypairs[PAYER],
                &pool_pda,
                &lp_token_mint_pda,
            )
            .await
            .unwrap(),
            1_128_110
        );
    }

    // Decrease asset price and check that lp token price decrease
    {
        // Makes ETH price to decrease of 20%
        {
            let eth_test_oracle_pda = custodies_infos[1].test_oracle_pda;
            let eth_custody_pda = custodies_infos[1].custody_pda;

            let publish_time = utils::get_current_unix_timestamp(&mut program_test_ctx).await;

            instructions::test_set_test_oracle_price(
                &mut program_test_ctx,
                &keypairs[MULTISIG_MEMBER_A],
                &keypairs[PAYER],
                &pool_pda,
                &eth_custody_pda,
                &eth_test_oracle_pda,
                SetTestOraclePriceParams {
                    price: utils::scale(1_320, ETH_DECIMALS),
                    expo: -(ETH_DECIMALS as i32),
                    conf: utils::scale(10, ETH_DECIMALS),
                    publish_time,
                },
                multisig_signers,
            )
            .await
            .unwrap();
        }

        assert_eq!(
            instructions::test_get_lp_token_price(
                &mut program_test_ctx,
                &keypairs[PAYER],
                &pool_pda,
                &lp_token_mint_pda,
            )
            .await
            .unwrap(),
            1_009_921
        );
    }
}
