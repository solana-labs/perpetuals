use {
    crate::utils::{self, pda},
    anchor_lang::ToAccountMetas,
    perpetuals::state::staking::Staking,
    solana_program::pubkey::Pubkey,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn test_init_staking(
    program_test_ctx: &mut ProgramTestContext,
    owner: &Keypair,
    payer: &Keypair,
) -> std::result::Result<(Pubkey, u8), BanksClientError> {
    // ==== GIVEN =============================================================
    let (staking_pda, staking_bump) = pda::get_staking_pda(&owner.pubkey());
    let staking_thread_authority_pda = pda::get_staking_thread_authority(&owner.pubkey()).0;

    // ==== WHEN ==============================================================

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::InitStaking {
            owner: owner.pubkey(),
            staking: staking_pda,
            staking_thread_authority: staking_thread_authority_pda,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::InitStaking {},
        Some(&payer.pubkey()),
        &[owner, payer],
    )
    .await?;

    // ==== THEN ==============================================================
    let staking_account = utils::get_account::<Staking>(program_test_ctx, staking_pda).await;

    {
        assert_eq!(staking_account.bump, staking_bump);
        assert_eq!(staking_account.locked_stakes.len(), 0);
        assert_eq!(staking_account.liquid_stake.amount, 0);
    }

    Ok((staking_pda, staking_bump))
}
