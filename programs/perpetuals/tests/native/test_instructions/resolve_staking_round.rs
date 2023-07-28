use {
    crate::utils::{self, pda},
    anchor_lang::{prelude::Pubkey, ToAccountMetas},
    perpetuals::state::staking::Staking,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    tokio::sync::RwLock,
};

pub async fn resolve_staking_round(
    program_test_ctx: &RwLock<ProgramTestContext>,
    caller: &Keypair,
    _owner: &Keypair,
    payer: &Keypair,
    staked_token_mint: &Pubkey,
) -> std::result::Result<(), BanksClientError> {
    // ==== GIVEN =============================================================
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
    let staking_pda = pda::get_staking_pda(staked_token_mint).0;
    let staking_staked_token_vault_pda = pda::get_staking_staked_token_vault_pda(&staking_pda).0;
    let staking_reward_token_vault_pda = pda::get_staking_reward_token_vault_pda(&staking_pda).0;
    let staking_lm_reward_token_vault_pda =
        pda::get_staking_lm_reward_token_vault_pda(&staking_pda).0;

    let staking_account = utils::get_account::<Staking>(program_test_ctx, staking_pda).await;

    // // ==== WHEN ==============================================================

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::ResolveStakingRound {
            caller: caller.pubkey(),
            staking_staked_token_vault: staking_staked_token_vault_pda,
            staking_reward_token_vault: staking_reward_token_vault_pda,
            staking_lm_reward_token_vault: staking_lm_reward_token_vault_pda,
            transfer_authority: transfer_authority_pda,
            staking: staking_pda,
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            lm_token_mint: lm_token_mint_pda,
            staking_reward_token_mint: staking_account.reward_token_mint,
            perpetuals_program: perpetuals::ID,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::ResolveStakingRound {},
        Some(&payer.pubkey()),
        &[caller, payer],
        None,
        None,
    )
    .await?;

    // // ==== THEN ==============================================================

    Ok(())
}