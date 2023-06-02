use {
    crate::{instructions, utils},
    maplit::hashmap,
    perpetuals::instructions::{AddLiquidityParams, RemoveLiquidityParams},
    solana_sdk::signer::Signer,
};

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;

pub async fn min_max_ratio() {
    let test_setup = utils::TestSetup::new(
        vec![utils::UserParam {
            name: "alice",
            token_balances: hashmap! {
                "usdc" => utils::scale(100_000, USDC_DECIMALS),
                "eth" => utils::scale(50, ETH_DECIMALS),
            },
        }],
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
            utils::NamedSetupCustodyWithLiquidityParams {
                setup_custody_params: utils::NamedSetupCustodyParams {
                    mint_name: "usdc",
                    is_stable: true,
                    is_virtual: false,
                    target_ratio: utils::ratio_from_percentage(50.0),
                    min_ratio: utils::ratio_from_percentage(30.0),
                    max_ratio: utils::ratio_from_percentage(60.0),
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
            utils::NamedSetupCustodyWithLiquidityParams {
                setup_custody_params: utils::NamedSetupCustodyParams {
                    mint_name: "eth",
                    is_stable: false,
                    is_virtual: false,
                    target_ratio: utils::ratio_from_percentage(50.0),
                    min_ratio: utils::ratio_from_percentage(30.0),
                    max_ratio: utils::ratio_from_percentage(60.0),
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

    let usdc_mint = &test_setup.get_mint_by_name("usdc");

    // Go over 60% ratio should trigger error
    assert!(instructions::test_add_liquidity(
        &mut test_setup.program_test_ctx.borrow_mut(),
        alice,
        &test_setup.payer_keypair,
        &test_setup.pool_pda,
        &usdc_mint,
        AddLiquidityParams {
            amount_in: utils::scale(1_000, USDC_DECIMALS),
            min_lp_amount_out: 1
        },
    )
    .await
    .is_err());

    let alice_lp_token_mint_pda =
        utils::find_associated_token_account(&alice.pubkey(), &test_setup.lp_token_mint_pda).0;

    let alice_lp_token_account_balance = utils::get_token_account_balance(
        &mut test_setup.program_test_ctx.borrow_mut(),
        alice_lp_token_mint_pda,
    )
    .await;

    // Try to remove 35% of LP token as USDC (~1,050 USDC), lowering USDC ratio to ~23%
    // Going under 30% ratio should trigger error
    assert!(instructions::test_remove_liquidity(
        &mut test_setup.program_test_ctx.borrow_mut(),
        alice,
        &test_setup.payer_keypair,
        &test_setup.pool_pda,
        &usdc_mint,
        RemoveLiquidityParams {
            lp_amount_in: alice_lp_token_account_balance * 35 / 100,
            min_amount_out: 1
        },
    )
    .await
    .is_err());
}
