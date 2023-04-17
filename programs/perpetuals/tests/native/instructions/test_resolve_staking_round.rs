use num::Zero;

use {
    crate::utils::{self, pda},
    anchor_lang::{prelude::Pubkey, ToAccountMetas},
    bonfida_test_utils::ProgramTestContextExt,
    perpetuals::state::cortex::Cortex,
    solana_program_test::BanksClientError,
    solana_program_test::ProgramTestContext,
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn test_resolve_staking_round(
    program_test_ctx: &mut ProgramTestContext,
    caller: &Keypair,
    owner: &Keypair,
    payer: &Keypair,
    stake_reward_token_mint: &Pubkey,
) -> std::result::Result<(), BanksClientError> {
    // ==== GIVEN =============================================================
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let stake_token_account_pda = pda::get_stake_token_account_pda().0;
    let stake_reward_token_account_pda = pda::get_stake_reward_token_account_pda().0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;

    let owner_lm_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lm_token_mint_pda).0;
    let caller_lm_token_account_address =
        utils::find_associated_token_account(&caller.pubkey(), &lm_token_mint_pda).0;

    // // ==== WHEN ==============================================================
    // save account state before tx execution
    let cortex_account_before = utils::get_account::<Cortex>(program_test_ctx, cortex_pda).await;
    let owner_lm_token_account_before = program_test_ctx
        .get_token_account(owner_lm_token_account_address)
        .await
        .unwrap();
    let caller_lm_token_account_before = program_test_ctx
        .get_token_account(caller_lm_token_account_address)
        .await
        .unwrap();

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::ResolveStakingRound {
            caller: caller.pubkey(),
            stake_token_account: stake_token_account_pda,
            stake_reward_token_account: stake_reward_token_account_pda,
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

    // ==== THEN ==============================================================

    // check staked balance unchanged
    {
        let owner_lm_token_account_after = program_test_ctx
            .get_token_account(owner_lm_token_account_address)
            .await
            .unwrap();
        let caller_lm_token_account_after = program_test_ctx
            .get_token_account(owner_lm_token_account_address)
            .await
            .unwrap();

        assert_eq!(
            owner_lm_token_account_after.amount,
            owner_lm_token_account_before.amount
        );
        assert_eq!(
            caller_lm_token_account_after.amount,
            caller_lm_token_account_before.amount
        );
    }

    // check `Cortex` data update
    {
        let cortex_account_after = utils::get_account::<Cortex>(program_test_ctx, cortex_pda).await;
        // the round is kept until all rewards are claimed. Empty rounds are never stored
        if !cortex_account_before
            .current_staking_round
            .total_stake
            .is_zero()
        {
            // updated amount of resolved staking rounds
            assert_eq!(
                cortex_account_after.resolved_staking_rounds.len(),
                cortex_account_before.resolved_staking_rounds.len() + 1
            );
            // last resolved round is the previous current round
            let latest_resolved_round =
                cortex_account_after.resolved_staking_rounds.last().unwrap();
            assert_eq!(
                latest_resolved_round.start_time,
                cortex_account_before.current_staking_round.start_time
            );
            assert_eq!(
                latest_resolved_round.total_stake,
                cortex_account_before.current_staking_round.total_stake
            );
        }
        // updated current staking round
        assert_ne!(
            cortex_account_after.current_staking_round,
            cortex_account_before.current_staking_round
        );
        // updated next staking round
        assert_ne!(
            cortex_account_after.next_staking_round,
            cortex_account_before.next_staking_round
        );
    }

    Ok(())
}
