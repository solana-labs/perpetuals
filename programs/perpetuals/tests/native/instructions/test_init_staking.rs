use {
    crate::utils::{self, pda},
    anchor_lang::ToAccountMetas,
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

    // // ==== WHEN ==============================================================

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::InitStaking {
            owner: owner.pubkey(),
            staking: staking_pda,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::InitStaking {},
        Some(&payer.pubkey()),
        &[owner, payer],
    )
    .await?;

    // ==== THEN ==============================================================

    Ok((staking_pda, staking_bump))
}
