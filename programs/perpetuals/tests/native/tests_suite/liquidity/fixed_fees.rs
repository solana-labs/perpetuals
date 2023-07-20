use {
    crate::{test_instructions, utils},
    maplit::hashmap,
    perpetuals::{
        instructions::{AddLiquidityParams, RemoveLiquidityParams},
        state::{cortex::Cortex, custody::Custody, perpetuals::Perpetuals, pool::Pool},
    },
};

const USDC_DECIMALS: u8 = 6;

pub async fn fixed_fees() {
    let test_setup = utils::TestSetup::new(
        vec![utils::UserParam {
            name: "alice",
            token_balances: hashmap! {
                "usdc" => utils::scale(100_000, USDC_DECIMALS),
            },
        }],
        vec![utils::MintParam {
            name: "usdc",
            decimals: USDC_DECIMALS,
        }],
        vec!["admin_a", "admin_b", "admin_c"],
        "usdc",
        "usdc",
        6,
        "ADRENA",
        "main_pool",
        vec![utils::SetupCustodyWithLiquidityParams {
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
            liquidity_amount: utils::scale(0, USDC_DECIMALS),
            payer_user_name: "alice",
        }],
        utils::scale(1_000_000, Cortex::LM_DECIMALS),
        utils::scale(1_000_000, Cortex::LM_DECIMALS),
        utils::scale(1_000_000, Cortex::LM_DECIMALS),
        utils::scale(1_000_000, Cortex::LM_DECIMALS),
    )
    .await;

    let alice = test_setup.get_user_keypair_by_name("alice");
    let usdc_mint = &test_setup.get_mint_by_name("usdc");

    // Check add liquidity fee
    {
        test_instructions::add_liquidity(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            usdc_mint,
            AddLiquidityParams {
                amount_in: utils::scale(1_000, USDC_DECIMALS),
                min_lp_amount_out: 1,
            },
        )
        .await
        .unwrap();

        {
            let pool_account =
                utils::get_account::<Pool>(&test_setup.program_test_ctx, test_setup.pool_pda).await;
            let custody_account = utils::get_account::<Custody>(
                &test_setup.program_test_ctx,
                test_setup.custodies_info[0].custody_pda,
            )
            .await;

            assert_eq!(
                pool_account.aum_usd,
                utils::scale_f64(993.965, USDC_DECIMALS).into(),
            );

            assert_eq!(
                custody_account.collected_fees.add_liquidity_usd,
                utils::scale(20, USDC_DECIMALS),
            );

            assert_eq!(
                custody_account.assets.protocol_fees,
                utils::scale_f64(0.05, USDC_DECIMALS),
            );
        }
    }

    // Check remove liquidity fee
    {
        test_instructions::remove_liquidity(
            &test_setup.program_test_ctx,
            alice,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            usdc_mint,
            RemoveLiquidityParams {
                lp_amount_in: utils::scale(100, Perpetuals::LP_DECIMALS),
                min_amount_out: 1,
            },
        )
        .await
        .unwrap();

        {
            let pool_account =
                utils::get_account::<Pool>(&test_setup.program_test_ctx, test_setup.pool_pda).await;
            let custody_account = utils::get_account::<Custody>(
                &test_setup.program_test_ctx,
                test_setup.custodies_info[0].custody_pda,
            )
            .await;

            assert_eq!(
                pool_account.aum_usd,
                utils::scale_f64(896.789201, USDC_DECIMALS).into(),
            );

            assert_eq!(
                custody_account.collected_fees.remove_liquidity_usd,
                utils::scale_f64(3.042750, USDC_DECIMALS),
            );

            assert_eq!(
                custody_account.assets.protocol_fees,
                utils::scale_f64(0.057607, USDC_DECIMALS),
            );
        }
    }
}
