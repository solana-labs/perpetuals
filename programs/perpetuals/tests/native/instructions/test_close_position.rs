use {
    super::get_update_pool_ix,
    crate::utils::{self, pda},
    anchor_lang::{prelude::Pubkey, ToAccountMetas},
    perpetuals::{
        instructions::ClosePositionParams,
        state::{custody::Custody, position::Position},
    },
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    tokio::sync::RwLock,
};

pub async fn test_close_position(
    program_test_ctx: &RwLock<ProgramTestContext>,
    owner: &Keypair,
    payer: &Keypair,
    position_pda: &Pubkey,
    params: ClosePositionParams,
) -> std::result::Result<(), BanksClientError> {
    // ==== WHEN ==============================================================
    let position_account = utils::get_account::<Position>(program_test_ctx, *position_pda).await;
    let custody_pda = position_account.custody;
    let collateral_custody_pda = position_account.collateral_custody;

    let collateral_custody_account =
        utils::get_account::<Custody>(program_test_ctx, collateral_custody_pda).await;

    // Prepare PDA and addresses
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let perpetuals_pda: Pubkey = pda::get_perpetuals_pda().0;
    let collateral_custody_token_account = pda::get_custody_token_account_pda(
        &position_account.pool,
        &collateral_custody_account.mint,
    )
    .0;

    let receiving_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &collateral_custody_account.mint).0;

    let custody_account = utils::get_account::<Custody>(program_test_ctx, custody_pda).await;
    let custody_oracle_account_address = custody_account.oracle.oracle_account;
    let collateral_custody_oracle_account_address =
        collateral_custody_account.oracle.oracle_account;

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::ClosePosition {
            owner: owner.pubkey(),
            receiving_account: receiving_account_address,
            transfer_authority: transfer_authority_pda,
            perpetuals: perpetuals_pda,
            pool: position_account.pool,
            position: *position_pda,
            custody: custody_pda,
            custody_oracle_account: custody_oracle_account_address,
            collateral_custody: collateral_custody_pda,
            collateral_custody_oracle_account: collateral_custody_oracle_account_address,
            collateral_custody_token_account,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::ClosePosition { params },
        Some(&payer.pubkey()),
        &[owner, payer],
        Some(get_update_pool_ix(program_test_ctx, payer, &position_account.pool).await?),
        Some(get_update_pool_ix(program_test_ctx, payer, &position_account.pool).await?),
    )
    .await?;

    // ==== THEN ==============================================================

    Ok(())
}
