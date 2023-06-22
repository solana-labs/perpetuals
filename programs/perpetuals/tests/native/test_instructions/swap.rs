use {
    crate::utils::{self, pda},
    anchor_lang::{prelude::Pubkey, ToAccountMetas},
    bonfida_test_utils::ProgramTestContextExt,
    perpetuals::{instructions::SwapParams, state::custody::Custody},
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn swap(
    program_test_ctx: &mut ProgramTestContext,
    owner: &Keypair,
    payer: &Keypair,
    pool_pda: &Pubkey,
    // Mint received by the User
    dispensing_custody_token_mint: &Pubkey,
    // Mint sent by the User
    receiving_custody_token_mint: &Pubkey,
    staking_reward_token_mint: &Pubkey,
    params: SwapParams,
) -> std::result::Result<(), BanksClientError> {
    // ==== WHEN ==============================================================
    // Prepare PDA and addresses
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let dispensing_custody_pda = pda::get_custody_pda(pool_pda, dispensing_custody_token_mint).0;
    let dispensing_custody_token_account_pda =
        pda::get_custody_token_account_pda(pool_pda, dispensing_custody_token_mint).0;
    let receiving_custody_pda = pda::get_custody_pda(pool_pda, receiving_custody_token_mint).0;
    let receiving_custody_token_account_pda =
        pda::get_custody_token_account_pda(pool_pda, receiving_custody_token_mint).0;
    let cortex_pda = pda::get_cortex_pda().0;
    let staking_pda = pda::get_staking_pda().0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;

    let funding_account_address =
        utils::find_associated_token_account(&owner.pubkey(), receiving_custody_token_mint).0;
    let receiving_account_address =
        utils::find_associated_token_account(&owner.pubkey(), dispensing_custody_token_mint).0;
    let lm_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lm_token_mint_pda).0;

    let dispensing_custody_account =
        utils::get_account::<Custody>(program_test_ctx, dispensing_custody_pda).await;
    let dispensing_custody_oracle_account_address =
        dispensing_custody_account.oracle.oracle_account;

    let receiving_custody_account =
        utils::get_account::<Custody>(program_test_ctx, receiving_custody_pda).await;
    let receiving_custody_oracle_account_address = receiving_custody_account.oracle.oracle_account;

    let staking_reward_token_vault_pda = pda::get_staking_reward_token_vault_pda().0;

    let srt_custody_pda = pda::get_custody_pda(pool_pda, staking_reward_token_mint).0;
    let srt_custody_token_account_pda =
        pda::get_custody_token_account_pda(pool_pda, staking_reward_token_mint).0;
    let srt_custody_account =
        utils::get_account::<Custody>(program_test_ctx, srt_custody_pda).await;
    let srt_custody_oracle_account_address = srt_custody_account.oracle.oracle_account;

    // Save account state before tx execution
    let owner_funding_account_before = program_test_ctx
        .get_token_account(funding_account_address)
        .await
        .unwrap();
    let owner_lm_token_account_before = program_test_ctx
        .get_token_account(lm_token_account_address)
        .await
        .unwrap();
    let custody_receiving_account_before = program_test_ctx
        .get_token_account(receiving_account_address)
        .await
        .unwrap();

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::Swap {
            owner: owner.pubkey(),
            funding_account: funding_account_address,
            receiving_account: receiving_account_address,
            lm_token_account: lm_token_account_address,
            transfer_authority: transfer_authority_pda,
            staking: staking_pda,
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            pool: *pool_pda,
            receiving_custody: receiving_custody_pda,
            receiving_custody_oracle_account: receiving_custody_oracle_account_address,
            receiving_custody_token_account: receiving_custody_token_account_pda,
            dispensing_custody: dispensing_custody_pda,
            dispensing_custody_oracle_account: dispensing_custody_oracle_account_address,
            dispensing_custody_token_account: dispensing_custody_token_account_pda,
            stake_reward_token_custody: srt_custody_pda,
            stake_reward_token_custody_oracle_account: srt_custody_oracle_account_address,
            stake_reward_token_custody_token_account: srt_custody_token_account_pda, // the stake reward vault
            staking_reward_token_vault: staking_reward_token_vault_pda,
            lm_token_mint: lm_token_mint_pda,
            staking_reward_token_mint: *staking_reward_token_mint,
            token_program: anchor_spl::token::ID,
            perpetuals_program: perpetuals::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::Swap { params },
        Some(&payer.pubkey()),
        &[owner, payer],
    )
    .await?;

    // ==== THEN ==============================================================
    // Check the balance change
    let owner_funding_account_after = program_test_ctx
        .get_token_account(funding_account_address)
        .await
        .unwrap();
    let owner_lm_token_account_after = program_test_ctx
        .get_token_account(lm_token_account_address)
        .await
        .unwrap();
    let custody_receiving_account_after = program_test_ctx
        .get_token_account(receiving_account_address)
        .await
        .unwrap();

    assert!(owner_funding_account_after.amount < owner_funding_account_before.amount);
    assert!(owner_lm_token_account_after.amount > owner_lm_token_account_before.amount);
    assert!(custody_receiving_account_after.amount > custody_receiving_account_before.amount);

    Ok(())
}
