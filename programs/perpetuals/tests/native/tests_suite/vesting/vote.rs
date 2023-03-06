use {
    crate::adapters,
    crate::utils::pda,
    crate::{
        instructions,
        utils::{self, fixtures},
    },
    perpetuals::{
        instructions::AddVestParams,
        state::{cortex::Cortex, perpetuals::Perpetuals},
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

const KEYPAIRS_COUNT: usize = 7;
const LM_TOKEN_DECIMALS: u8 = 6;

#[tokio::test]
pub async fn vote() {
    let mut program_test = ProgramTest::default();

    // Initialize the accounts that will be used during the test suite
    let keypairs =
        utils::create_and_fund_multiple_accounts(&mut program_test, KEYPAIRS_COUNT).await;

    // Deploy programs
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

    instructions::test_init(
        &mut program_test_ctx,
        upgrade_authority,
        fixtures::init_params_permissions_full(1),
        &governance_realm_pda,
        multisig_signers,
    )
    .await
    .unwrap();

    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;

    let realm_pda = adapters::spl_governance::create_realm(
        &mut program_test_ctx,
        &keypairs[ROOT_AUTHORITY],
        &keypairs[PAYER],
        "ADRENA".to_string(),
        utils::scale(10_000, LM_TOKEN_DECIMALS),
        &lm_token_mint_pda,
    )
    .await
    .unwrap();

    // Initialize and fund associated token accounts
    {
        // Alice: create LM token account
        {
            utils::initialize_token_account(
                &mut program_test_ctx,
                &lm_token_mint_pda,
                &keypairs[USER_ALICE].pubkey(),
            )
            .await;
        }
    }

    // Alice: vest 1m token, unlockable at 50% unlock share
    let alice_vest_pda = instructions::test_add_vest(
        &mut program_test_ctx,
        &keypairs[MULTISIG_MEMBER_A],
        &keypairs[PAYER],
        &keypairs[USER_ALICE],
        &governance_realm_pda,
        &AddVestParams {
            amount: utils::scale(1_000_000, Cortex::LM_DECIMALS),
            unlock_share: utils::scale_f64(0.5, Perpetuals::BPS_DECIMALS),
        },
        multisig_signers,
    )
    .await
    .unwrap()
    .0;

    let governance_pda = adapters::spl_governance::create_governance(
        &mut program_test_ctx,
        &alice_vest_pda,
        &keypairs[USER_ALICE],
        &keypairs[PAYER],
        &realm_pda,
        &lm_token_mint_pda,
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
        &mut program_test_ctx,
        &keypairs[PAYER],
        "Test Proposal".to_string(),
        "Description".to_string(),
        &realm_pda,
        &governance_pda,
        &lm_token_mint_pda,
        &alice_vest_pda,
        &keypairs[USER_ALICE],
    )
    .await
    .unwrap();

    adapters::spl_governance::cast_vote(
        &mut program_test_ctx,
        &keypairs[PAYER],
        &realm_pda,
        &governance_pda,
        &proposal_pda,
        &lm_token_mint_pda,
        &alice_vest_pda,
        &alice_vest_pda,
        &keypairs[USER_ALICE],
        true,
    )
    .await
    .unwrap();

    adapters::spl_governance::cancel_proposal(
        &mut program_test_ctx,
        &keypairs[PAYER],
        &realm_pda,
        &governance_pda,
        &proposal_pda,
        &lm_token_mint_pda,
        &alice_vest_pda,
        &keypairs[USER_ALICE],
    )
    .await
    .unwrap();

    adapters::spl_governance::relinquish_vote(
        &mut program_test_ctx,
        &keypairs[PAYER],
        &realm_pda,
        &governance_pda,
        &proposal_pda,
        &lm_token_mint_pda,
        &alice_vest_pda,
        &keypairs[USER_ALICE],
    )
    .await
    .unwrap();

    // Alice: claim vest
    instructions::test_claim_vest(
        &mut program_test_ctx,
        &keypairs[PAYER],
        &keypairs[USER_ALICE],
        &governance_realm_pda,
    )
    .await
    .unwrap();
}
