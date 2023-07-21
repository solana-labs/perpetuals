use {
    crate::{test_instructions, utils},
    maplit::hashmap,
    perpetuals::{
        instructions::{BucketName, MintLmTokensFromBucketParams},
        state::cortex::Cortex,
    },
    solana_sdk::signer::Signer,
};

const USDC_DECIMALS: u8 = 6;
const ETH_DECIMALS: u8 = 9;

pub async fn mint_lm_tokens_from_bucket() {
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
        "usdc",
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
                liquidity_amount: utils::scale(0, USDC_DECIMALS),
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
                liquidity_amount: utils::scale(0, ETH_DECIMALS),
                payer_user_name: "alice",
            },
        ],
        utils::scale(100_000, Cortex::LM_DECIMALS),
        utils::scale(200_000, Cortex::LM_DECIMALS),
        utils::scale(300_000, Cortex::LM_DECIMALS),
        utils::scale(500_000, Cortex::LM_DECIMALS),
    )
    .await;

    let alice = test_setup.get_user_keypair_by_name("alice");

    let admin_a = test_setup.get_multisig_member_keypair_by_name("admin_a");

    let multisig_signers = test_setup.get_multisig_signers();

    test_instructions::mint_lm_tokens_from_bucket(
        &test_setup.program_test_ctx,
        admin_a,
        &alice.pubkey(),
        &test_setup.payer_keypair,
        MintLmTokensFromBucketParams {
            bucket_name: BucketName::CoreContributor,
            amount: utils::scale(50_000, Cortex::LM_DECIMALS),
            reason: "Mint 50% of core contributor bucket allocation".to_string(),
        },
        &multisig_signers,
    )
    .await
    .unwrap();

    assert!(test_instructions::mint_lm_tokens_from_bucket(
        &test_setup.program_test_ctx,
        admin_a,
        &alice.pubkey(),
        &test_setup.payer_keypair,
        MintLmTokensFromBucketParams {
            bucket_name: BucketName::CoreContributor,
            amount: utils::scale(60_000, Cortex::LM_DECIMALS),
            reason: "Mint 60% of core contributor bucket allocation should fail as we already minted 50%".to_string(),
        },
        &multisig_signers,
    )
    .await
    .is_err());

    test_instructions::mint_lm_tokens_from_bucket(
        &test_setup.program_test_ctx,
        admin_a,
        &alice.pubkey(),
        &test_setup.payer_keypair,
        MintLmTokensFromBucketParams {
            bucket_name: BucketName::CoreContributor,
            amount: utils::scale(50_000, Cortex::LM_DECIMALS),
            reason: "Mint other 50% of core contributor bucket allocation".to_string(),
        },
        &multisig_signers,
    )
    .await
    .unwrap();

    test_instructions::mint_lm_tokens_from_bucket(
        &test_setup.program_test_ctx,
        admin_a,
        &alice.pubkey(),
        &test_setup.payer_keypair,
        MintLmTokensFromBucketParams {
            bucket_name: BucketName::DaoTreasury,
            amount: utils::scale(200_000, Cortex::LM_DECIMALS),
            reason: "Mint 100% of dao treasury bucket allocation".to_string(),
        },
        &multisig_signers,
    )
    .await
    .unwrap();

    assert!(test_instructions::mint_lm_tokens_from_bucket(
        &test_setup.program_test_ctx,
        admin_a,
        &alice.pubkey(),
        &test_setup.payer_keypair,
        MintLmTokensFromBucketParams {
            bucket_name: BucketName::Ecosystem,
            amount: utils::scale(0, Cortex::LM_DECIMALS),
            reason: "Mint 0 should fail".to_string(),
        },
        &multisig_signers,
    )
    .await
    .is_err());
}
