use {
    crate::{instructions, utils},
    maplit::hashmap,
    perpetuals::{
        instructions::{ClosePositionParams, OpenPositionParams, SetTestOraclePriceParams},
        state::{custody::PricingParams, position::Side},
    },
    solana_sdk::signer::Signer,
};

const ETH_DECIMALS: u8 = 9;
const USDC_DECIMALS: u8 = 6;

pub async fn max_user_profit() {
    let test_setup = utils::TestSetup::new(
        vec![
            utils::UserParam {
                name: "alice",
                token_balances: hashmap! {
                    "usdc" => utils::scale(1_000, USDC_DECIMALS),
                    "eth" => utils::scale(10_000, ETH_DECIMALS),
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
                    target_ratio: utils::ratio_from_percentage(100.0),
                    min_ratio: utils::ratio_from_percentage(0.0),
                    max_ratio: utils::ratio_from_percentage(100.0),
                    initial_price: utils::scale(1_500, ETH_DECIMALS),
                    initial_conf: utils::scale(10, ETH_DECIMALS),
                    pricing_params: Some(PricingParams {
                        // Expressed in BPS, with BPS = 10_000
                        // 2_500 = x0.25, 10_000 = x1, 50_000 = x5
                        max_payoff_mult: 2_500,
                        ..utils::fixtures::pricing_params_regular(false)
                    }),
                    permissions: None,
                    fees: None,
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale(10_000, ETH_DECIMALS),
                payer_user_name: "alice",
            },
        ],
    )
    .await;

    let martin = test_setup.get_user_keypair_by_name("martin");

    let admin_a = test_setup.get_multisig_member_keypair_by_name("admin_a");

    let multisig_signers = test_setup.get_multisig_signers();

    let eth_mint = &test_setup.get_mint_by_name("eth");

    // Martin: Open 1 ETH long position x5
    let position_pda = instructions::test_open_position(
        &mut test_setup.program_test_ctx.borrow_mut(),
        martin,
        &test_setup.payer_keypair,
        &test_setup.pool_pda,
        &eth_mint,
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
        let eth_test_oracle_pda = test_setup.custodies_info[1].test_oracle_pda;
        let eth_custody_pda = test_setup.custodies_info[1].custody_pda;

        let publish_time =
            utils::get_current_unix_timestamp(&mut test_setup.program_test_ctx.borrow_mut()).await;

        instructions::test_set_test_oracle_price(
            &mut test_setup.program_test_ctx.borrow_mut(),
            admin_a,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            &eth_custody_pda,
            &eth_test_oracle_pda,
            SetTestOraclePriceParams {
                price: utils::scale(3_000, ETH_DECIMALS),
                expo: -(ETH_DECIMALS as i32),
                conf: utils::scale(10, ETH_DECIMALS),
                publish_time,
            },
            &multisig_signers,
        )
        .await
        .unwrap();
    }

    utils::warp_forward(&mut test_setup.program_test_ctx.borrow_mut(), 1).await;

    instructions::test_close_position(
        &mut test_setup.program_test_ctx.borrow_mut(),
        martin,
        &test_setup.payer_keypair,
        &test_setup.pool_pda,
        &eth_mint,
        &position_pda,
        ClosePositionParams {
            // lowest exit price paid (slippage implied)
            price: utils::scale(2_970, USDC_DECIMALS),
        },
    )
    .await
    .unwrap();

    utils::warp_forward(&mut test_setup.program_test_ctx.borrow_mut(), 1).await;

    // Check user gains
    {
        let martin_eth_pda = utils::find_associated_token_account(&martin.pubkey(), &eth_mint).0;

        let martin_eth_balance = utils::get_token_account_balance(
            &mut test_setup.program_test_ctx.borrow_mut(),
            martin_eth_pda,
        )
        .await;

        // Gains are limited to 0.25 * 5 = 1.25 ETH
        // True gains should be 2.5 ETH less fees (price did x2 on x5 leverage)
        assert_eq!(martin_eth_balance, utils::scale_f64(2.7, ETH_DECIMALS));
    }
}
