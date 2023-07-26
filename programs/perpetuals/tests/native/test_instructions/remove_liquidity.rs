use {
    crate::utils::{self, pda},
    anchor_lang::{
        prelude::{AccountMeta, Pubkey},
        ToAccountMetas,
    },
    perpetuals::{
        instructions::RemoveLiquidityParams,
        state::{custody::Custody, pool::Pool, staking::Staking},
    },
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    tokio::sync::RwLock,
};

pub async fn remove_liquidity(
    program_test_ctx: &RwLock<ProgramTestContext>,
    owner: &Keypair,
    payer: &Keypair,
    pool_pda: &Pubkey,
    custody_token_mint: &Pubkey,
    params: RemoveLiquidityParams,
) -> std::result::Result<(), BanksClientError> {
    // ==== WHEN ==============================================================

    // Prepare PDA and addresses
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let custody_pda = pda::get_custody_pda(pool_pda, custody_token_mint).0;
    let custody_token_account_pda =
        pda::get_custody_token_account_pda(pool_pda, custody_token_mint).0;
    let lp_token_mint_pda = pda::get_lp_token_mint_pda(pool_pda).0;
    let cortex_pda = pda::get_cortex_pda().0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
    let lm_staking_pda = pda::get_staking_pda(&lm_token_mint_pda).0;
    let lp_staking_pda = pda::get_staking_pda(&lp_token_mint_pda).0;
    let receiving_account_address =
        utils::find_associated_token_account(&owner.pubkey(), custody_token_mint).0;
    let lp_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lp_token_mint_pda).0;
    let lm_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lm_token_mint_pda).0;

    let custody_account = utils::get_account::<Custody>(program_test_ctx, custody_pda).await;
    let custody_oracle_account_address = custody_account.oracle.oracle_account;

    let lm_staking_reward_token_vault_pda =
        pda::get_staking_reward_token_vault_pda(&lm_staking_pda).0;

    let lp_staking_reward_token_vault_pda =
        pda::get_staking_reward_token_vault_pda(&lp_staking_pda).0;

    let lm_staking_account = utils::get_account::<Staking>(program_test_ctx, lm_staking_pda).await;

    let srt_custody_pda = pda::get_custody_pda(pool_pda, &lm_staking_account.reward_token_mint).0;
    let srt_custody_token_account_pda =
        pda::get_custody_token_account_pda(pool_pda, &lm_staking_account.reward_token_mint).0;
    let srt_custody_account =
        utils::get_account::<Custody>(program_test_ctx, srt_custody_pda).await;
    let srt_custody_oracle_account_address = srt_custody_account.oracle.oracle_account;

    // Save account state before tx execution
    let owner_receiving_account_before =
        utils::get_token_account(program_test_ctx, receiving_account_address).await;
    let owner_lp_token_account_before =
        utils::get_token_account(program_test_ctx, lp_token_account_address).await;
    let owner_lm_token_account_before =
        utils::get_token_account(program_test_ctx, lm_token_account_address).await;
    let custody_token_account_before =
        utils::get_token_account(program_test_ctx, custody_token_account_pda).await;

    let accounts_meta = {
        let accounts = perpetuals::accounts::RemoveLiquidity {
            owner: owner.pubkey(),
            receiving_account: receiving_account_address,
            lp_token_account: lp_token_account_address,
            lm_token_account: lm_token_account_address,
            transfer_authority: transfer_authority_pda,
            lm_staking: lm_staking_pda,
            lp_staking: lp_staking_pda,
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            pool: *pool_pda,
            custody: custody_pda,
            custody_oracle_account: custody_oracle_account_address,
            custody_token_account: custody_token_account_pda,
            staking_reward_token_custody: srt_custody_pda,
            staking_reward_token_custody_oracle_account: srt_custody_oracle_account_address,
            staking_reward_token_custody_token_account: srt_custody_token_account_pda,
            lm_staking_reward_token_vault: lm_staking_reward_token_vault_pda, // the stake reward vault
            lp_staking_reward_token_vault: lp_staking_reward_token_vault_pda, // the stake reward vault
            lp_token_mint: lp_token_mint_pda,
            lm_token_mint: lm_token_mint_pda,
            staking_reward_token_mint: lm_staking_account.reward_token_mint,
            token_program: anchor_spl::token::ID,
            perpetuals_program: perpetuals::ID,
        };

        let mut accounts_meta = accounts.to_account_metas(None);

        let pool_account = utils::get_account::<Pool>(program_test_ctx, *pool_pda).await;

        // For each token, add custody account as remaining_account
        for custody in &pool_account.custodies {
            accounts_meta.push(AccountMeta {
                pubkey: *custody,
                is_signer: false,
                is_writable: false,
            });
        }

        // For each token, add custody oracle account as remaining_account
        for custody in &pool_account.custodies {
            let custody_account = utils::get_account::<Custody>(program_test_ctx, *custody).await;

            accounts_meta.push(AccountMeta {
                pubkey: custody_account.oracle.oracle_account,
                is_signer: false,
                is_writable: false,
            });
        }

        accounts_meta
    };

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        accounts_meta,
        perpetuals::instruction::RemoveLiquidity { params },
        Some(&payer.pubkey()),
        &[owner, payer],
        None,
        None,
    )
    .await?;

    // ==== THEN ==============================================================
    let owner_receiving_account_after =
        utils::get_token_account(program_test_ctx, receiving_account_address).await;
    let owner_lp_token_account_after =
        utils::get_token_account(program_test_ctx, lp_token_account_address).await;
    let owner_lm_token_account_after =
        utils::get_token_account(program_test_ctx, lm_token_account_address).await;
    let custody_token_account_after =
        utils::get_token_account(program_test_ctx, custody_token_account_pda).await;

    assert!(owner_receiving_account_after.amount > owner_receiving_account_before.amount);
    assert!(owner_lp_token_account_after.amount < owner_lp_token_account_before.amount);
    assert!(owner_lm_token_account_after.amount > owner_lm_token_account_before.amount);
    assert!(custody_token_account_after.amount < custody_token_account_before.amount);

    Ok(())
}
