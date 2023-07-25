use {
    super::get_update_pool_ix,
    crate::utils::{self, pda},
    anchor_lang::{prelude::Pubkey, ToAccountMetas},
    perpetuals::{
        instructions::OpenPositionParams,
        state::{custody::Custody, position::Position},
    },
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    tokio::sync::RwLock,
};

pub async fn test_open_position(
    program_test_ctx: &RwLock<ProgramTestContext>,
    owner: &Keypair,
    payer: &Keypair,
    pool_pda: &Pubkey,
    custody_token_mint: &Pubkey,
    params: OpenPositionParams,
) -> std::result::Result<(solana_sdk::pubkey::Pubkey, u8), BanksClientError> {
    // ==== WHEN ==============================================================

    // Prepare PDA and addresses
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let custody_pda = pda::get_custody_pda(pool_pda, custody_token_mint).0;
    let custody_token_account_pda =
        pda::get_custody_token_account_pda(pool_pda, custody_token_mint).0;

    let (position_pda, position_bump) =
        pda::get_position_pda(&owner.pubkey(), pool_pda, &custody_pda, params.side);

    let funding_account_address =
        utils::find_associated_token_account(&owner.pubkey(), custody_token_mint).0;

    let custody_account = utils::get_account::<Custody>(program_test_ctx, custody_pda).await;
    let custody_oracle_account_address = custody_account.oracle.oracle_account;

    // Save account state before tx execution
    let owner_funding_account_before =
        utils::get_token_account(program_test_ctx, funding_account_address).await;
    let custody_token_account_before =
        utils::get_token_account(program_test_ctx, custody_token_account_pda).await;

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::OpenPosition {
            owner: owner.pubkey(),
            funding_account: funding_account_address,
            transfer_authority: transfer_authority_pda,
            perpetuals: perpetuals_pda,
            pool: *pool_pda,
            position: position_pda,
            custody: custody_pda,
            custody_oracle_account: custody_oracle_account_address,
            collateral_custody: custody_pda,
            collateral_custody_oracle_account: custody_oracle_account_address,
            collateral_custody_token_account: custody_token_account_pda,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::OpenPosition { params },
        Some(&payer.pubkey()),
        &[owner, payer],
        Some(get_update_pool_ix(program_test_ctx, payer, pool_pda).await?),
        None,
    )
    .await?;

    // ==== THEN ==============================================================
    // Check the balance change
    {
        let owner_funding_account_after =
            utils::get_token_account(program_test_ctx, funding_account_address).await;
        let custody_token_account_after =
            utils::get_token_account(program_test_ctx, custody_token_account_pda).await;

        assert!(owner_funding_account_after.amount < owner_funding_account_before.amount);
        assert!(custody_token_account_after.amount > custody_token_account_before.amount);
    }

    // Check the position
    {
        let position_account = utils::get_account::<Position>(program_test_ctx, position_pda).await;

        assert_eq!(position_account.owner, owner.pubkey());
        assert_eq!(position_account.pool, *pool_pda);
        assert_eq!(position_account.custody, custody_pda);
        // Need to handle test/not test case
        // assert_eq!(
        //     position_account.open_time,
        //     utils::get_current_unix_timestamp(program_test_ctx).await
        // );
        assert_eq!(position_account.update_time, 0);
        assert_eq!(position_account.side, params.side);
        assert_eq!(position_account.unrealized_profit_usd, 0);
        assert_eq!(position_account.unrealized_loss_usd, 0);
        assert_eq!(position_account.collateral_amount, params.collateral);
        assert_eq!(position_account.bump, position_bump);
    }

    Ok((position_pda, position_bump))
}
