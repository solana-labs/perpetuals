use {
    crate::{
        assert_unchanged,
        instructions::{test_close_position, test_open_position, test_set_custom_oracle_price},
        utils,
    },
    maplit::hashmap,
    perpetuals::{
        instructions::{ClosePositionParams, OpenPositionParams, SetCustomOraclePriceParams},
        state::{
            custody::{Custody, PricingParams},
            perpetuals::Perpetuals,
            position::{Position, Side},
        },
    },
    solana_sdk::signer::Signer,
};

const ETH_DECIMALS: u8 = 9;
const USDC_DECIMALS: u8 = 6;

#[allow(deprecated)]
pub async fn open_and_close_long_position_accounting() {
    let test_setup = utils::TestSetup::new(
        vec![
            utils::UserParam {
                name: "alice",
                token_balances: hashmap! {
                    "usdc" => utils::scale(150_000, USDC_DECIMALS),
                    "eth" => utils::scale(100, ETH_DECIMALS),
                },
            },
            utils::UserParam {
                name: "martin",
                token_balances: hashmap! {
                    "usdc" => utils::scale(150_000, USDC_DECIMALS),
                    "eth" => utils::scale(100, ETH_DECIMALS),
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
                liquidity_amount: utils::scale(150_000, USDC_DECIMALS),
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
                        // 50_000 = x5, 100_000 = x10
                        max_leverage: 100_000,
                        ..utils::fixtures::pricing_params_regular(false)
                    }),
                    permissions: None,
                    fees: None,
                    borrow_rate: None,
                },
                liquidity_amount: utils::scale(100, ETH_DECIMALS),
                payer_user_name: "alice",
            },
        ],
    )
    .await;

    let martin = test_setup.get_user_keypair_by_name("martin");

    let admin_a = test_setup.get_multisig_member_keypair_by_name("admin_a");

    let multisig_signers = test_setup.get_multisig_signers();

    let eth_mint = &test_setup.get_mint_by_name("eth");

    let martin_eth_ata =
        utils::find_associated_token_account(&martin.try_pubkey().unwrap(), eth_mint).0;

    let eth_custody_pda = test_setup.custodies_info[1].custody_pda;

    let eth_custody_account_before =
        utils::get_account::<Custody>(&test_setup.program_test_ctx, eth_custody_pda).await;

    let martin_eth_ata_balance_before =
        utils::get_token_account_balance(&test_setup.program_test_ctx, martin_eth_ata).await;

    // Martin: Open 1 ETH long position x5
    let position_pda = test_open_position(
        &test_setup.program_test_ctx,
        martin,
        &test_setup.payer_keypair,
        &test_setup.pool_pda,
        eth_mint,
        None,
        OpenPositionParams {
            // max price paid (slippage implied)
            price: utils::scale(1_550, Perpetuals::USD_DECIMALS),
            collateral: utils::scale(1, ETH_DECIMALS),
            size: utils::scale(5, ETH_DECIMALS),
            side: Side::Long,
        },
    )
    .await
    .unwrap()
    .0;

    {
        let eth_custody_account_after =
            utils::get_account::<Custody>(&test_setup.program_test_ctx, eth_custody_pda).await;
        let martin_eth_ata_balance_after =
            utils::get_token_account_balance(&test_setup.program_test_ctx, martin_eth_ata).await;

        // Check user balance
        {
            assert_eq!(
                // Paid 1 ETH as collateral and 0.05 ETH as fees
                martin_eth_ata_balance_before - 1_050_000_000,
                martin_eth_ata_balance_after
            );
        }

        // Check the position PDA info
        {
            let position =
                utils::get_account::<Position>(&test_setup.program_test_ctx, position_pda).await;

            assert_eq!(position.side, Side::Long);
            // entry price
            // price of the token + trade_spread_long (in BPS)
            assert_eq!(position.price, 1_515_000_000);
            // locked amount * position price (entry price)
            assert_eq!(position.size_usd, 7_575_000_000);
            assert_eq!(position.borrow_size_usd, 7_575_000_000);
            // 1 ETH at price
            assert_eq!(position.collateral_usd, 1_500_000_000);
            // 1 ETH
            assert_eq!(position.collateral_amount, 1_000_000_000);
            assert_eq!(position.unrealized_profit_usd, 0);
            assert_eq!(position.unrealized_loss_usd, 0);
            assert_eq!(position.cumulative_interest_snapshot, 0);
            // 5 ETH
            assert_eq!(position.locked_amount, 5_000_000_000);
        }

        // Double check effect of opening position on ETH custody accounting
        {
            // Collected fees
            {
                let before = &eth_custody_account_before.collected_fees;
                let after = &eth_custody_account_after.collected_fees;

                assert_eq!(
                    before.open_position_usd + 75_000_000,
                    after.open_position_usd
                );

                assert_unchanged!(before.swap_usd, after.swap_usd);
                assert_unchanged!(before.add_liquidity_usd, after.add_liquidity_usd);
                assert_unchanged!(before.remove_liquidity_usd, after.remove_liquidity_usd);
                assert_unchanged!(before.close_position_usd, after.close_position_usd);
                assert_unchanged!(before.liquidation_usd, after.liquidation_usd);
            }

            // Volume stats
            {
                let before = &eth_custody_account_before.volume_stats;
                let after = &eth_custody_account_after.volume_stats;

                assert_eq!(
                    before.open_position_usd + 7_575_000_000,
                    after.open_position_usd
                );

                assert_unchanged!(before.swap_usd, after.swap_usd);
                assert_unchanged!(before.add_liquidity_usd, after.add_liquidity_usd);
                assert_unchanged!(before.remove_liquidity_usd, after.remove_liquidity_usd);
                assert_unchanged!(before.close_position_usd, after.close_position_usd);
                assert_unchanged!(before.liquidation_usd, after.liquidation_usd);
            }

            // Trade Stats
            {
                let before = &eth_custody_account_before.trade_stats;
                let after = &eth_custody_account_after.trade_stats;

                assert_eq!(before.oi_long_usd + 7_575_000_000, after.oi_long_usd);

                assert_unchanged!(before.profit_usd, after.profit_usd);
                assert_unchanged!(before.loss_usd, after.loss_usd);
                assert_unchanged!(before.oi_short_usd, after.oi_short_usd);
            }

            // Long positions
            {
                let before = &eth_custody_account_before.long_positions;
                let after = &eth_custody_account_after.long_positions;

                assert_eq!(before.open_positions + 1, after.open_positions);

                assert_eq!(before.size_usd + 7_575_000_000, after.size_usd);

                assert_eq!(
                    before.borrow_size_usd + 7_575_000_000,
                    after.borrow_size_usd
                );

                assert_eq!(
                    // 5 ETH
                    before.locked_amount + 5_000_000_000,
                    after.locked_amount
                );

                assert_eq!(before.total_quantity + 50_000, after.total_quantity);

                // WeightedPrice = position_price * quantity
                assert_eq!(
                    before.weighted_price + 75_750_000_000_000,
                    after.weighted_price
                );

                // Should probably change, mark the parameter as deprecated
                assert_unchanged!(before.collateral_usd, after.collateral_usd);

                assert_unchanged!(
                    before.cumulative_interest_usd,
                    after.cumulative_interest_usd
                );

                assert_unchanged!(
                    before.cumulative_interest_snapshot,
                    after.cumulative_interest_snapshot
                );
            }

            // Short positions
            {
                let before = &eth_custody_account_before.short_positions;
                let after = &eth_custody_account_after.short_positions;

                assert_unchanged!(before.open_positions, after.open_positions);
                assert_unchanged!(before.collateral_usd, after.collateral_usd);
                assert_unchanged!(before.size_usd, after.size_usd);
                assert_unchanged!(before.borrow_size_usd, after.borrow_size_usd);
                assert_unchanged!(before.locked_amount, after.locked_amount);
                assert_unchanged!(before.weighted_price, after.weighted_price);
                assert_unchanged!(before.total_quantity, after.total_quantity);
                assert_unchanged!(
                    before.cumulative_interest_usd,
                    after.cumulative_interest_usd
                );
                assert_unchanged!(
                    before.cumulative_interest_snapshot,
                    after.cumulative_interest_snapshot
                );
            }
        }
    }

    // Wait for 10 hours so we can see the borrow rate in action
    utils::warp_forward(&test_setup.program_test_ctx, 36_000).await;

    // Makes ETH price to drop 10%
    {
        let eth_test_oracle_pda = test_setup.custodies_info[1].custom_oracle_pda;
        let eth_custody_pda = test_setup.custodies_info[1].custody_pda;

        let publish_time = utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await;

        test_set_custom_oracle_price(
            &test_setup.program_test_ctx,
            admin_a,
            &test_setup.payer_keypair,
            &test_setup.pool_pda,
            &eth_custody_pda,
            &eth_test_oracle_pda,
            SetCustomOraclePriceParams {
                price: utils::scale(1_350, ETH_DECIMALS),
                expo: -(ETH_DECIMALS as i32),
                conf: utils::scale(10, ETH_DECIMALS),
                ema: utils::scale(1_350, ETH_DECIMALS),
                publish_time,
            },
            &multisig_signers,
        )
        .await
        .unwrap();
    }

    let eth_custody_account_before =
        utils::get_account::<Custody>(&test_setup.program_test_ctx, eth_custody_pda).await;

    let martin_eth_ata_balance_before =
        utils::get_token_account_balance(&test_setup.program_test_ctx, martin_eth_ata).await;

    // Martin: Close the ETH position
    test_close_position(
        &test_setup.program_test_ctx,
        martin,
        &test_setup.payer_keypair,
        &position_pda,
        ClosePositionParams {
            // lower the price for slippage
            price: utils::scale(1_330, Perpetuals::USD_DECIMALS),
        },
    )
    .await
    .unwrap();

    {
        let eth_custody_account_after =
            utils::get_account::<Custody>(&test_setup.program_test_ctx, eth_custody_pda).await;
        let martin_eth_ata_balance_after =
            utils::get_token_account_balance(&test_setup.program_test_ctx, martin_eth_ata).await;

        // Check user balance
        {
            // User provided 1 ETH at $1,500
            // ETH lost 10% of value, and now worth $1,350
            // User position was x5 leverage
            // User lost (1,500-1,350)*5 = $750 in absolute value
            //
            // User is at $178,5 lost per ETH (price diff)
            // entry_position_price - exit_price (both taking into account spread) & borrow
            //
            // Fees: $75,750001
            // Total amount lost: $968.250016 (178,5 * 5 + 75,750001)
            //
            // User get back ~$531,74 worth of ETH (1,500 original value minus ~$968 loss)
            assert_eq!(
                martin_eth_ata_balance_before + 393_608_332,
                martin_eth_ata_balance_after
            );
        }

        // Double check effect of closing position on ETH custody accounting
        {
            // Collected fees
            {
                let before = &eth_custody_account_before.collected_fees;
                let after = &eth_custody_account_after.collected_fees;

                assert_eq!(
                    before.close_position_usd + 75_750_001,
                    after.close_position_usd
                );

                assert_unchanged!(before.open_position_usd, after.open_position_usd);
                assert_unchanged!(before.swap_usd, after.swap_usd);
                assert_unchanged!(before.add_liquidity_usd, after.add_liquidity_usd);
                assert_unchanged!(before.remove_liquidity_usd, after.remove_liquidity_usd);
                assert_unchanged!(before.liquidation_usd, after.liquidation_usd);
            }

            // Volume stats
            {
                let before = &eth_custody_account_before.volume_stats;
                let after = &eth_custody_account_after.volume_stats;

                assert_eq!(
                    // locked amount (size) * position price (entry price)
                    before.close_position_usd + 7_575_000_000,
                    after.close_position_usd
                );

                assert_unchanged!(before.swap_usd, before.swap_usd);
                assert_unchanged!(before.open_position_usd, after.open_position_usd);
                assert_unchanged!(before.add_liquidity_usd, after.add_liquidity_usd);
                assert_unchanged!(before.remove_liquidity_usd, after.remove_liquidity_usd);
                assert_unchanged!(before.liquidation_usd, after.liquidation_usd);
            }

            // Trade Stats
            {
                let before = &eth_custody_account_before.trade_stats;
                let after = &eth_custody_account_after.trade_stats;

                assert_eq!(
                    // locked amount (size) * position price (entry price)
                    before.oi_long_usd - 7_575_000_000,
                    after.oi_long_usd
                );

                assert_eq!(before.loss_usd + 968_628_751, after.loss_usd);

                assert_unchanged!(before.profit_usd, after.profit_usd);
                assert_unchanged!(before.oi_short_usd, after.oi_short_usd);
            }

            // Long positions
            {
                let before = &eth_custody_account_before.long_positions;
                let after = &eth_custody_account_after.long_positions;

                assert_eq!(before.open_positions - 1, after.open_positions);

                assert_eq!(
                    // locked amount (size) * position price (entry price)
                    before.size_usd - 7_575_000_000,
                    after.size_usd
                );

                assert_eq!(
                    // locked amount (size) * position price (entry price)
                    before.borrow_size_usd - 7_575_000_000,
                    after.borrow_size_usd
                );

                assert_eq!(
                    // 5 ETH
                    before.locked_amount - 5_000_000_000,
                    after.locked_amount
                );

                assert_eq!(before.total_quantity - 50_000, after.total_quantity);

                assert_eq!(
                    before.weighted_price - 75_750_000_000_000,
                    after.weighted_price
                );

                assert_unchanged!(
                    before.cumulative_interest_snapshot,
                    after.cumulative_interest_snapshot
                );

                assert_unchanged!(before.collateral_usd, after.collateral_usd);

                assert_unchanged!(
                    before.cumulative_interest_usd,
                    after.cumulative_interest_usd
                );
            }

            // Short positions
            {
                let before = &eth_custody_account_before.short_positions;
                let after = &eth_custody_account_after.short_positions;

                assert_unchanged!(before.open_positions, after.open_positions);
                assert_unchanged!(before.collateral_usd, after.collateral_usd);
                assert_unchanged!(before.size_usd, after.size_usd);
                assert_unchanged!(before.borrow_size_usd, after.borrow_size_usd);
                assert_unchanged!(before.locked_amount, after.locked_amount);
                assert_unchanged!(before.weighted_price, after.weighted_price);
                assert_unchanged!(before.total_quantity, after.total_quantity);

                assert_unchanged!(
                    before.cumulative_interest_usd,
                    after.cumulative_interest_usd
                );

                assert_unchanged!(
                    before.cumulative_interest_snapshot,
                    after.cumulative_interest_snapshot
                );
            }
        }
    }
}
