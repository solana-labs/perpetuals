use {
    crate::{instructions, utils},
    maplit::hashmap,
    perpetuals::{
        instructions::{
            ClosePositionParams, OpenPositionParams, RemoveLiquidityParams, SwapParams,
        },
        state::position::Side,
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
    let paul = test_setup.get_user_keypair_by_name("paul");

    let usdc_mint = &test_setup.get_mint_by_name("usdc");
    let eth_mint = &test_setup.get_mint_by_name("eth");

    // Simple open/close position
    {
        // Martin: Open 0.1 ETH position
        let position_pda = instructions::test_open_position(
            &mut test_setup.program_test_ctx.borrow_mut(),
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
        instructions::test_close_position(
            &mut test_setup.program_test_ctx.borrow_mut(),
            martin,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
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

    // Simple swap
    {
        // Paul: Swap 150 USDC for ETH
        instructions::test_swap(
            &mut test_setup.program_test_ctx.borrow_mut(),
            paul,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            &eth_mint,
            // The program receives USDC
            &usdc_mint,
            SwapParams {
                amount_in: utils::scale(150, USDC_DECIMALS),

                // 1% slippage
                min_amount_out: utils::scale(150, USDC_DECIMALS)
                    / utils::scale(1_500, ETH_DECIMALS)
                    * 99
                    / 100,
            },
        )
        .await
        .unwrap();
    }

    // Remove liquidity
    {
        let alice_lp_token =
            utils::find_associated_token_account(&alice.pubkey(), &test_setup.lp_token_mint_pda).0;

        let alice_lp_token_balance = utils::get_token_account_balance(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice_lp_token,
        )
        .await;

        // Alice: Remove 100% of provided liquidity (1k USDC less fees)
        instructions::test_remove_liquidity(
            &mut test_setup.program_test_ctx.borrow_mut(),
            alice,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            &usdc_mint,
            RemoveLiquidityParams {
                lp_amount_in: alice_lp_token_balance,
                min_amount_out: 1,
            },
        )
        .await
        .unwrap();
    }
}
