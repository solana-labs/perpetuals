use {
    crate::utils::{self, pda},
    anchor_lang::{prelude::Pubkey, ToAccountMetas},
    bonfida_test_utils::ProgramTestContextExt,
    perpetuals::state::stake::Stake,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn test_claim_stake(
    program_test_ctx: &mut ProgramTestContext,
    caller: &Keypair,
    owner: &Keypair,
    payer: &Keypair,
    stake_reward_token_mint: &Pubkey,
) -> std::result::Result<(), BanksClientError> {
    // ==== GIVEN =============================================================
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let (stake_pda, stake_bump) = pda::get_stake_pda(&owner.pubkey());
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let stake_reward_token_account_pda = pda::get_stake_reward_token_account_pda().0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;

    let owner_lm_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lm_token_mint_pda).0;
    let owner_stake_reward_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &stake_reward_token_mint).0;
    let caller_stake_reward_token_account_address =
        utils::find_associated_token_account(&caller.pubkey(), &stake_reward_token_mint).0;

    // // ==== WHEN ==============================================================
    // Note: skip the test if there is no stake account
    if let Some(stake_account_before) =
        utils::try_get_account::<Stake>(program_test_ctx, stake_pda).await
    {
        let owner_lm_token_account_before = program_test_ctx
            .get_token_account(owner_lm_token_account_address)
            .await
            .unwrap();

        utils::create_and_execute_perpetuals_ix(
            program_test_ctx,
            perpetuals::accounts::ClaimStake {
                caller: caller.pubkey(),
                owner: owner.pubkey(),
                caller_reward_token_account: caller_stake_reward_token_account_address,
                owner_reward_token_account: owner_stake_reward_token_account_address,
                stake_reward_token_account: stake_reward_token_account_pda,
                transfer_authority: transfer_authority_pda,
                stake: stake_pda,
                cortex: cortex_pda,
                perpetuals: perpetuals_pda,
                lm_token_mint: lm_token_mint_pda,
                stake_reward_token_mint: *stake_reward_token_mint,
                system_program: anchor_lang::system_program::ID,
                token_program: anchor_spl::token::ID,
            }
            .to_account_metas(None),
            perpetuals::instruction::ClaimStake {},
            Some(&payer.pubkey()),
            &[caller, payer],
        )
        .await?;

        // ==== THEN ==============================================================

        // check staked balance unchanged
        {
            let owner_lm_token_account_after = program_test_ctx
                .get_token_account(owner_lm_token_account_address)
                .await
                .unwrap();
            assert_eq!(
                owner_lm_token_account_after.amount,
                owner_lm_token_account_before.amount
            );
        }

        // check `Stake` data update
        {
            let stake_account_after =
                utils::get_account::<Stake>(program_test_ctx, stake_pda).await;
            assert_eq!(stake_account_after.amount, stake_account_before.amount);
            assert_eq!(stake_account_after.bump, stake_bump);
        }
    }
    Ok(())
}
