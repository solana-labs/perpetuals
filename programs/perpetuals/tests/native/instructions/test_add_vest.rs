use {
    crate::utils::{self, pda},
    anchor_lang::{
        prelude::{AccountMeta, Pubkey},
        ToAccountMetas,
    },
    perpetuals::{
        adapters::spl_governance_program_adapter,
        instructions::AddVestParams,
        state::{cortex::Cortex, multisig::Multisig, vest::Vest},
    },
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn test_add_vest(
    program_test_ctx: &mut ProgramTestContext,
    // Admin must be a part of the multisig
    admin: &Keypair,
    payer: &Keypair,
    owner: &Keypair,
    governance_realm_pda: &Pubkey,
    params: &AddVestParams,
    multisig_signers: &[&Keypair],
) -> std::result::Result<(Pubkey, u8), BanksClientError> {
    // ==== WHEN ==============================================================
    let multisig_pda = pda::get_multisig_pda().0;
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let (vest_pda, vest_bump) = pda::get_vest_pda(&owner.pubkey());
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
    let governance_token_mint_pda = pda::get_governance_token_mint_pda().0;

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

    let multisig_account = utils::get_account::<Multisig>(program_test_ctx, multisig_pda).await;

    // Before state
    let governance_governing_token_holding_balance_before =
        utils::get_token_account_balance(program_test_ctx, governance_governing_token_holding_pda)
            .await;

    // One Tx per multisig signer
    for i in 0..multisig_account.min_signatures {
        let signer: &Keypair = multisig_signers[i as usize];

        let accounts_meta = {
            let accounts = perpetuals::accounts::AddVest {
                admin: admin.pubkey(),
                owner: owner.pubkey(),
                payer: payer.pubkey(),
                multisig: multisig_pda,
                transfer_authority: transfer_authority_pda,
                cortex: cortex_pda,
                perpetuals: perpetuals_pda,
                vest: vest_pda,
                lm_token_mint: lm_token_mint_pda,
                governance_token_mint: governance_token_mint_pda,
                governance_realm: *governance_realm_pda,
                governance_realm_config: governance_realm_config_pda,
                governance_governing_token_holding: governance_governing_token_holding_pda,
                governance_governing_token_owner_record:
                    governance_governing_token_owner_record_pda,
                governance_program: spl_governance_program_adapter::ID,
                system_program: anchor_lang::system_program::ID,
                token_program: anchor_spl::token::ID,
                rent: solana_program::sysvar::rent::ID,
            };

            let mut accounts_meta = accounts.to_account_metas(None);

            accounts_meta.push(AccountMeta {
                pubkey: signer.pubkey(),
                is_signer: true,
                is_writable: false,
            });

            accounts_meta
        };

        utils::create_and_execute_perpetuals_ix(
            program_test_ctx,
            accounts_meta,
            perpetuals::instruction::AddVest {
                params: AddVestParams {
                    amount: params.amount,
                    unlock_start_timestamp: params.unlock_start_timestamp,
                    unlock_end_timestamp: params.unlock_end_timestamp,
                },
            },
            Some(&payer.pubkey()),
            &[admin, payer, signer],
        )
        .await?;
    }

    // ==== THEN ==============================================================

    // Check vest account
    {
        let vest_account = utils::get_account::<Vest>(program_test_ctx, vest_pda).await;

        assert_eq!(vest_account.amount, params.amount);
        assert_eq!(
            vest_account.unlock_start_timestamp,
            params.unlock_start_timestamp
        );
        assert_eq!(
            vest_account.unlock_end_timestamp,
            params.unlock_end_timestamp
        );
        assert_eq!(vest_account.claimed_amount, 0);
        assert_eq!(vest_account.last_claim_timestamp, 0);
        assert_eq!(vest_account.owner, owner.pubkey());
        assert_eq!(vest_account.bump, vest_bump);
    }

    // Check cortex account
    {
        let cortex_account = utils::get_account::<Cortex>(program_test_ctx, cortex_pda).await;

        assert_eq!(*cortex_account.vests.last().unwrap(), vest_pda);
    }

    // Check governance accounts
    {
        let governance_governing_token_holding_balance_after = utils::get_token_account_balance(
            program_test_ctx,
            governance_governing_token_holding_pda,
        )
        .await;

        assert_eq!(
            governance_governing_token_holding_balance_before + params.amount,
            governance_governing_token_holding_balance_after
        );
    }

    Ok((vest_pda, vest_bump))
}
