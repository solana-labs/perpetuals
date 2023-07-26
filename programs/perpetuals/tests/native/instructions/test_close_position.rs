use {
    super::get_update_pool_ix,
    crate::utils::{self, pda},
    anchor_lang::{prelude::Pubkey, ToAccountMetas},
    perpetuals::{instructions::ClosePositionParams, state::custody::Custody},
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    tokio::sync::RwLock,
};

pub async fn test_close_position(
    program_test_ctx: &RwLock<ProgramTestContext>,
    owner: &Keypair,
    payer: &Keypair,
    pool_pda: &Pubkey,
    custody_token_mint: &Pubkey,
    position_pda: &Pubkey,
    params: ClosePositionParams,
) -> std::result::Result<(), BanksClientError> {
    // ==== WHEN ==============================================================

    // Prepare PDA and addresses
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let custody_pda = pda::get_custody_pda(pool_pda, custody_token_mint).0;
    let custody_token_account_pda =
        pda::get_custody_token_account_pda(pool_pda, custody_token_mint).0;

    let receiving_account_address =
        utils::find_associated_token_account(&owner.pubkey(), custody_token_mint).0;

    let custody_account = utils::get_account::<Custody>(program_test_ctx, custody_pda).await;
    let custody_oracle_account_address = custody_account.oracle.oracle_account;

    // Save account state before tx execution
    let owner_receiving_account_before =
        utils::get_token_account(program_test_ctx, receiving_account_address).await;

    let custody_token_account_before =
        utils::get_token_account(program_test_ctx, custody_token_account_pda).await;

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::ClosePosition {
            owner: owner.pubkey(),
            receiving_account: receiving_account_address,
            transfer_authority: transfer_authority_pda,
            perpetuals: perpetuals_pda,
            pool: *pool_pda,
            position: *position_pda,
            custody: custody_pda,
            custody_oracle_account: custody_oracle_account_address,
            collateral_custody: custody_pda,
            collateral_custody_oracle_account: custody_oracle_account_address,
            collateral_custody_token_account: custody_token_account_pda,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::ClosePosition { params },
        Some(&payer.pubkey()),
        &[owner, payer],
        Some(get_update_pool_ix(program_test_ctx, payer, pool_pda).await?),
        Some(get_update_pool_ix(program_test_ctx, payer, pool_pda).await?),
    )
    .await?;

    // ==== THEN ==============================================================
    // Check the balance change
    {
        let owner_receiving_account_after =
            utils::get_token_account(program_test_ctx, receiving_account_address).await;
        let custody_token_account_after =
            utils::get_token_account(program_test_ctx, custody_token_account_pda).await;

        assert!(owner_receiving_account_after.amount > owner_receiving_account_before.amount);
        assert!(custody_token_account_after.amount < custody_token_account_before.amount);
    }

    Ok(())
}
