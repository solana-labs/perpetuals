use {
    crate::utils::{self, pda},
    anchor_lang::{prelude::Pubkey, ToAccountMetas},
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn resolve_staking_round(
    program_test_ctx: &mut ProgramTestContext,
    caller: &Keypair,
    _owner: &Keypair,
    payer: &Keypair,
    stake_reward_token_mint: &Pubkey,
) -> std::result::Result<(), BanksClientError> {
    // ==== GIVEN =============================================================
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let stake_token_account_pda = pda::get_stake_token_account_pda().0;
    let stake_reward_token_account_pda = pda::get_stake_reward_token_account_pda().0;
    let stake_lm_reward_token_account_pda = pda::get_stake_lm_reward_token_account_pda().0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;

    // // ==== WHEN ==============================================================

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::ResolveStakingRound {
            caller: caller.pubkey(),
            stake_token_account: stake_token_account_pda,
            stake_reward_token_account: stake_reward_token_account_pda,
            stake_lm_reward_token_account: stake_lm_reward_token_account_pda,
            transfer_authority: transfer_authority_pda,
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            lm_token_mint: lm_token_mint_pda,
            stake_reward_token_mint: *stake_reward_token_mint,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::ResolveStakingRound {},
        Some(&payer.pubkey()),
        &[caller, payer],
    )
    .await?;

    // // ==== THEN ==============================================================

    Ok(())
}
