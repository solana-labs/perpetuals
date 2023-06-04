use {
    crate::utils::{self, pda},
    anchor_lang::{prelude::Pubkey, ToAccountMetas},
    bonfida_test_utils::ProgramTestContextExt,
    perpetuals::{
        adapters::spl_governance_program_adapter,
        state::{cortex::Cortex, vest::Vest},
    },
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn test_claim_vest(
    program_test_ctx: &mut ProgramTestContext,
    payer: &Keypair,
    owner: &Keypair,
    governance_realm_pda: &Pubkey,
) -> std::result::Result<(), BanksClientError> {
    // ==== WHEN ==============================================================
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let vest_pda = pda::get_vest_pda(&owner.pubkey()).0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
    let governance_token_mint_pda = pda::get_governance_token_mint_pda().0;
    let lm_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lm_token_mint_pda).0;

    let governance_governing_token_holding_pda = pda::get_governance_governing_token_holding_pda(
        governance_realm_pda,
        &governance_token_mint_pda,
    );

    let governance_realm_config_pda = pda::get_governance_realm_config_pda(governance_realm_pda);

    let governance_governing_token_owner_record_pda =
        pda::get_governance_governing_token_owner_record_pda(
            governance_realm_pda,
            &governance_token_mint_pda,
            &owner.pubkey(),
        );

    // Save account state before tx execution
    let vest_account_before = utils::get_account::<Vest>(program_test_ctx, vest_pda).await;
    let owner_lm_token_account_before = program_test_ctx
        .get_token_account(lm_token_account_address)
        .await
        .unwrap();

    // Before state
    let governance_governing_token_holding_balance_before =
        utils::get_token_account_balance(program_test_ctx, governance_governing_token_holding_pda)
            .await;

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::ClaimVest {
            owner: owner.pubkey(),
            receiving_account: lm_token_account_address,
            transfer_authority: transfer_authority_pda,
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            vest: vest_pda,
            lm_token_mint: lm_token_mint_pda,
            governance_token_mint: governance_token_mint_pda,
            governance_realm: *governance_realm_pda,
            governance_realm_config: governance_realm_config_pda,
            governance_governing_token_holding: governance_governing_token_holding_pda,
            governance_governing_token_owner_record: governance_governing_token_owner_record_pda,
            governance_program: spl_governance_program_adapter::ID,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
            rent: solana_program::sysvar::rent::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::ClaimVest {},
        Some(&payer.pubkey()),
        &[payer, owner],
    )
    .await?;

    // ==== THEN ==============================================================

    let vest_account_after = utils::get_account::<Vest>(program_test_ctx, vest_pda).await;

    // Check user account received tokens
    {
        let owner_lm_token_account_after = program_test_ctx
            .get_token_account(lm_token_account_address)
            .await
            .unwrap();

        assert_eq!(
            owner_lm_token_account_after.amount,
            owner_lm_token_account_before.amount
                + (vest_account_after.claimed_amount - vest_account_before.claimed_amount)
        )
    }

    // If everything have been claimed, verify that the vest pda has been plucked out of the Cortex vests vector
    if vest_account_after.amount == vest_account_after.claimed_amount {
        {
            let cortex_account = utils::get_account::<Cortex>(program_test_ctx, cortex_pda).await;

            assert_eq!(
                cortex_account.vests.iter().find(|v| { **v == vest_pda }),
                None
            );
        }
    }

    // The governance power should be reduced
    {
        let governance_governing_token_holding_balance_after = utils::get_token_account_balance(
            program_test_ctx,
            governance_governing_token_holding_pda,
        )
        .await;

        assert_eq!(
            governance_governing_token_holding_balance_before
                - (vest_account_after.claimed_amount - vest_account_before.claimed_amount),
            governance_governing_token_holding_balance_after
        );
    }

    Ok(())
}
