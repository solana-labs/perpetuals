use {
    crate::{adapters, test_instructions, utils},
    maplit::hashmap,
    perpetuals::{instructions::AddVestParams, state::cortex::Cortex},
};

const USDC_DECIMALS: u8 = 6;

pub async fn vote() {
    let test_setup = utils::TestSetup::new(
        vec![utils::UserParam {
            name: "alice",
            token_balances: hashmap! {},
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

    let admin_a = test_setup.get_multisig_member_keypair_by_name("admin_a");

    let multisig_signers = test_setup.get_multisig_signers();

    // Alice: vest 1m token, unlock period from now to in 7 days
    let current_time = utils::get_current_unix_timestamp(&test_setup.program_test_ctx).await;

    let alice_vest_pda = test_instructions::add_vest(
        &test_setup.program_test_ctx,
        admin_a,
        &test_setup.payer_keypair,
        alice,
        &test_setup.governance_realm_pda,
        &AddVestParams {
            amount: utils::scale(1_000_000, Cortex::LM_DECIMALS),
            unlock_start_timestamp: current_time,
            unlock_end_timestamp: utils::days_in_seconds(7) + current_time,
        },
        &multisig_signers,
    )
    .await
    .unwrap()
    .0;

    let governance_pda = adapters::spl_governance::create_governance(
        &test_setup.program_test_ctx,
        &alice_vest_pda,
        alice,
        &test_setup.payer_keypair,
        &test_setup.governance_realm_pda,
        &test_setup.lm_token_mint,
        None,
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap()
    .0;

    let proposal_pda = adapters::spl_governance::create_proposal(
        &test_setup.program_test_ctx,
        &test_setup.payer_keypair,
        "Test Proposal".to_string(),
        "Description".to_string(),
        &test_setup.governance_realm_pda,
        &governance_pda,
        &test_setup.lm_token_mint,
        &alice_vest_pda,
        alice,
    )
    .await
    .unwrap();

    adapters::spl_governance::cast_vote(
        &test_setup.program_test_ctx,
        &test_setup.payer_keypair,
        &test_setup.governance_realm_pda,
        &governance_pda,
        &proposal_pda,
        &test_setup.lm_token_mint,
        &alice_vest_pda,
        &alice_vest_pda,
        alice,
        true,
    )
    .await
    .unwrap();

    adapters::spl_governance::cancel_proposal(
        &test_setup.program_test_ctx,
        &test_setup.payer_keypair,
        &test_setup.governance_realm_pda,
        &governance_pda,
        &proposal_pda,
        &test_setup.lm_token_mint,
        &alice_vest_pda,
        alice,
    )
    .await
    .unwrap();

    adapters::spl_governance::relinquish_vote(
        &test_setup.program_test_ctx,
        &test_setup.payer_keypair,
        &test_setup.governance_realm_pda,
        &governance_pda,
        &proposal_pda,
        &test_setup.lm_token_mint,
        &alice_vest_pda,
        alice,
    )
    .await
    .unwrap();

    // Alice: claim vest
    test_instructions::claim_vest(
        &test_setup.program_test_ctx,
        &test_setup.payer_keypair,
        alice,
        &test_setup.governance_realm_pda,
    )
    .await
    .unwrap();
}
