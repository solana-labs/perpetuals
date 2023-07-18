use {
    crate::{instructions, utils},
    maplit::hashmap,
    perpetuals::instructions::SwapParams,
};

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;

pub async fn insuffisient_fund() {
    let test_setup = utils::TestSetup::new(
        vec![
            utils::UserParam {
                name: "alice",
                token_balances: hashmap! {
                    "usdc" => utils::scale(7_500, USDC_DECIMALS),
                    "eth" => utils::scale(5, ETH_DECIMALS),
                },
            },
            utils::UserParam {
                name: "martin",
                token_balances: hashmap! {
                    "usdc" => utils::scale(1_000, USDC_DECIMALS),
                    "eth" => utils::scale(10, ETH_DECIMALS),
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
                liquidity_amount: utils::scale(7_500, USDC_DECIMALS),
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
                liquidity_amount: utils::scale(5, ETH_DECIMALS),
                payer_user_name: "alice",
            },
        ],
    )
    .await;

    let martin = test_setup.get_user_keypair_by_name("martin");

    let usdc_mint = &test_setup.get_mint_by_name("usdc");
    let eth_mint = &test_setup.get_mint_by_name("eth");

    // Swap with not enough collateral should fail
    {
        // Martin: Swap 5k USDC for ETH
        assert!(instructions::test_swap(
            &test_setup.program_test_ctx,
            martin,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            eth_mint,
            // The program receives USDC
            usdc_mint,
            SwapParams {
                amount_in: utils::scale(5_000, USDC_DECIMALS),
                min_amount_out: 0,
            },
        )
        .await
        .is_err());
    }

    // Swap for more token that the pool own should fail
    {
        // Martin: Swap 10 ETH for (15k) USDC
        assert!(instructions::test_swap(
            &test_setup.program_test_ctx,
            martin,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            usdc_mint,
            // The program receives ETH
            eth_mint,
            SwapParams {
                amount_in: utils::scale(10, ETH_DECIMALS),
                min_amount_out: 0,
            },
        )
        .await
        .is_err());
    }
}
