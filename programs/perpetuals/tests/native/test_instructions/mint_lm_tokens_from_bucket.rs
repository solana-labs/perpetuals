use {
    crate::utils::{self, pda},
    anchor_lang::{
        prelude::{AccountMeta, Pubkey},
        ToAccountMetas,
    },
    perpetuals::{instructions::MintLmTokensFromBucketParams, state::multisig::Multisig},
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    tokio::sync::RwLock,
};

pub async fn mint_lm_tokens_from_bucket(
    program_test_ctx: &RwLock<ProgramTestContext>,
    admin: &Keypair,
    owner: &Pubkey,
    payer: &Keypair,
    params: MintLmTokensFromBucketParams,
    multisig_signers: &[&Keypair],
) -> std::result::Result<(), BanksClientError> {
    // ==== WHEN ==============================================================
    let multisig_pda = pda::get_multisig_pda().0;
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
    let lm_token_account_address =
        utils::find_associated_token_account(owner, &lm_token_mint_pda).0;

    let multisig_account = utils::get_account::<Multisig>(program_test_ctx, multisig_pda).await;

    // One Tx per multisig signer
    for i in 0..multisig_account.min_signatures {
        let signer: &Keypair = multisig_signers[i as usize];

        let accounts_meta = {
            let accounts = perpetuals::accounts::MintLmTokensFromBucket {
                admin: admin.pubkey(),
                receiving_account: lm_token_account_address,
                transfer_authority: transfer_authority_pda,
                cortex: cortex_pda,
                perpetuals: perpetuals_pda,
                lm_token_mint: lm_token_mint_pda,
                token_program: anchor_spl::token::ID,
            };

            let mut accounts_meta = accounts.to_account_metas(None);

            // Add the multisig account
            accounts_meta.push(AccountMeta {
                pubkey: multisig_pda,
                is_signer: false,
                is_writable: true,
            });

            // Add the admin pubkey
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
            perpetuals::instruction::MintLmTokensFromBucket {
                params: params.clone(),
            },
            Some(&payer.pubkey()),
            &[admin, payer, signer],
            None,
            None,
        )
        .await?;
    }

    // ==== THEN ==============================================================

    Ok(())
}
