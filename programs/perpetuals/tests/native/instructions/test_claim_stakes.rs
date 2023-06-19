use {
    crate::utils::{self, pda},
    anchor_lang::{prelude::Pubkey, ToAccountMetas},
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn test_claim_stakes(
    program_test_ctx: &mut ProgramTestContext,
    caller: &Keypair,
    owner: &Keypair,
    payer: &Keypair,
    stake_reward_token_mint: &Pubkey,
) -> std::result::Result<(), BanksClientError> {
    // ==== GIVEN =============================================================
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let staking_pda = pda::get_staking_pda(&owner.pubkey()).0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let stake_reward_token_account_pda = pda::get_stake_reward_token_account_pda().0;

    let owner_stake_reward_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &stake_reward_token_mint).0;

    // ==== WHEN ==============================================================

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::ClaimStakes {
            caller: caller.pubkey(),
            owner: owner.pubkey(),
            payer: payer.pubkey(),
            owner_reward_token_account: owner_stake_reward_token_account_address,
            stake_reward_token_account: stake_reward_token_account_pda,
            transfer_authority: transfer_authority_pda,
            staking: staking_pda,
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            stake_reward_token_mint: *stake_reward_token_mint,
            perpetuals_program: perpetuals::ID,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::ClaimStakes {},
        Some(&payer.pubkey()),
        &[caller, payer],
    )
    .await?;

    // ==== THEN ==============================================================

    Ok(())
}
