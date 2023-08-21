use {
    crate::utils::{self, pda},
    anchor_lang::{
        prelude::{AccountMeta, Pubkey},
        AnchorSerialize, ToAccountMetas,
    },
    perpetuals::{
        adapters::spl_governance_program_adapter,
        instructions::AddGenesisLiquidityParams,
        state::{custody::Custody, pool::Pool, user_staking::UserStaking},
    },
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    tokio::sync::RwLock,
};

pub async fn add_genesis_liquidity(
    program_test_ctx: &RwLock<ProgramTestContext>,
    owner: &Keypair,
    payer: &Keypair,
    pool_pda: &Pubkey,
    custody_token_mint: &Pubkey,
    governance_realm_pda: &Pubkey,
    params: AddGenesisLiquidityParams,
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
    let lp_staking_pda = pda::get_staking_pda(&lp_token_mint_pda).0;
    let lp_user_staking_pda = pda::get_user_staking_pda(&owner.pubkey(), &lp_staking_pda).0;
    let lp_staking_staked_token_vault_pda =
        pda::get_staking_staked_token_vault_pda(&lp_staking_pda).0;
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

    let lp_user_staking_thread_authority_pda =
        pda::get_user_staking_thread_authority(&lp_user_staking_pda).0;
    let locked_stake_resolution_thread_address = pda::get_thread_address(
        &lp_user_staking_thread_authority_pda,
        params.lp_stake_resolution_thread_id.try_to_vec().unwrap(),
    );

    let funding_account_address =
        utils::find_associated_token_account(&owner.pubkey(), custody_token_mint).0;
    let lp_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lp_token_mint_pda).0;

    let custody_account = utils::get_account::<Custody>(program_test_ctx, custody_pda).await;
    let custody_oracle_account_address = custody_account.oracle.oracle_account;

    // Save account state before tx execution
    let lp_user_staking_account_before =
        utils::get_account::<UserStaking>(program_test_ctx, lp_user_staking_pda).await;

    let lp_stakes_claim_cron_thread_address = pda::get_thread_address(
        &lp_user_staking_thread_authority_pda,
        lp_user_staking_account_before
            .stakes_claim_cron_thread_id
            .try_to_vec()
            .unwrap(),
    );

    let owner_funding_account_before =
        utils::get_token_account(program_test_ctx, funding_account_address).await;
    let owner_lp_token_account_before =
        utils::get_token_account(program_test_ctx, lp_token_account_address).await;
    let custody_token_account_before =
        utils::get_token_account(program_test_ctx, custody_token_account_pda).await;

    let accounts_meta = {
        let accounts = perpetuals::accounts::AddGenesisLiquidity {
            owner: owner.pubkey(),
            funding_account: funding_account_address,
            lp_token_account: lp_token_account_address,
            transfer_authority: transfer_authority_pda,
            lp_staking: lp_staking_pda,
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            pool: *pool_pda,
            custody: custody_pda,
            custody_oracle_account: custody_oracle_account_address,
            custody_token_account: custody_token_account_pda,
            lp_token_mint: lp_token_mint_pda,
            lm_token_mint: lm_token_mint_pda,
            lp_user_staking: lp_user_staking_pda,
            lp_staking_staked_token_vault: lp_staking_staked_token_vault_pda,
            governance_token_mint: governance_token_mint_pda,
            governance_realm: *governance_realm_pda,
            governance_realm_config: governance_realm_config_pda,
            governance_governing_token_holding: governance_governing_token_holding_pda,
            governance_governing_token_owner_record: governance_governing_token_owner_record_pda,
            lp_stake_resolution_thread: locked_stake_resolution_thread_address,
            stakes_claim_cron_thread: lp_stakes_claim_cron_thread_address,
            lp_user_staking_thread_authority: lp_user_staking_thread_authority_pda,
            clockwork_program: clockwork_sdk::ID,
            governance_program: spl_governance_program_adapter::ID,
            perpetuals_program: perpetuals::ID,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
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
        perpetuals::instruction::AddGenesisLiquidity { params },
        Some(&payer.pubkey()),
        &[owner, payer],
        None,
        None,
    )
    .await?;

    // ==== THEN ==============================================================
    let owner_funding_account_after =
        utils::get_token_account(program_test_ctx, funding_account_address).await;
    let owner_lp_token_account_after =
        utils::get_token_account(program_test_ctx, lp_token_account_address).await;
    let custody_token_account_after =
        utils::get_token_account(program_test_ctx, custody_token_account_pda).await;

    assert!(owner_funding_account_after.amount < owner_funding_account_before.amount);
    assert!(owner_lp_token_account_after.amount == owner_lp_token_account_before.amount);
    assert!(custody_token_account_after.amount > custody_token_account_before.amount);

    Ok(())
}
