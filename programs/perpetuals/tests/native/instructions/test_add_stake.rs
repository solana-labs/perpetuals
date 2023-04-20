use {
    crate::utils::{self, pda},
    anchor_lang::{
        prelude::{Clock, Pubkey},
        ToAccountMetas,
    },
    bonfida_test_utils::ProgramTestContextExt,
    perpetuals::{
        instructions::AddStakeParams,
        state::{cortex::Cortex, stake::Stake},
    },
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn test_add_stake(
    program_test_ctx: &mut ProgramTestContext,
    owner: &Keypair,
    payer: &Keypair,
    params: AddStakeParams,
    stake_reward_token_mint: &Pubkey,
) -> std::result::Result<(), BanksClientError> {
    // ==== GIVEN =============================================================
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let (stake_pda, stake_bump) = pda::get_stake_pda(&owner.pubkey());
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let stake_token_account_pda = pda::get_stake_token_account_pda().0;
    let stake_reward_token_account_pda = pda::get_stake_reward_token_account_pda().0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;

    let lm_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lm_token_mint_pda).0;
    let stake_reward_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &stake_reward_token_mint).0;

    // // ==== WHEN ==============================================================
    // save account state before tx execution
    let cortex_account_before = utils::get_account::<Cortex>(program_test_ctx, cortex_pda).await;
    let stake_account_before = utils::try_get_account::<Stake>(program_test_ctx, stake_pda).await;
    let owner_lm_token_account_before = program_test_ctx
        .get_token_account(lm_token_account_address)
        .await
        .unwrap();
    // let owner_stake_reward_token_account_before = program_test_ctx
    //     .get_token_account(stake_reward_token_account_address)
    //     .await
    //     .unwrap();

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::AddStake {
            owner: owner.pubkey(),
            funding_account: lm_token_account_address,
            owner_reward_token_account: stake_reward_token_account_address,
            stake_token_account: stake_token_account_pda,
            stake_reward_token_account: stake_reward_token_account_pda,
            transfer_authority: transfer_authority_pda,
            stake: stake_pda,
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            lm_token_mint: lm_token_mint_pda,
            stake_reward_token_mint: *stake_reward_token_mint,
            perpetuals_program: perpetuals::ID,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::AddStake {
            params: AddStakeParams {
                amount: params.amount,
            },
        },
        Some(&payer.pubkey()),
        &[owner, payer],
    )
    .await?;

    // ==== THEN ==============================================================

    // check balance changes
    {
        let owner_lm_token_account_after = program_test_ctx
            .get_token_account(lm_token_account_address)
            .await
            .unwrap();

        assert_eq!(
            owner_lm_token_account_after.amount,
            owner_lm_token_account_before.amount - params.amount
        );
    }

    // check `Cortex` data update
    {
        let cortex_account_after = utils::get_account::<Cortex>(program_test_ctx, cortex_pda).await;
        // same amount of resolved staking rounds
        assert_eq!(
            cortex_account_after.resolved_staking_rounds.len(),
            cortex_account_before.resolved_staking_rounds.len()
        );
        // conditionnal checks if the account was initialized previously
        if let Some(s) = stake_account_before.clone() {
            // forfeited the previously staked amount for this round
            assert_eq!(
                cortex_account_after.current_staking_round.total_stake,
                cortex_account_before.current_staking_round.total_stake - s.amount
            );
        }
        // restaked the initial amount minus the removed amount for next round
        assert_eq!(
            cortex_account_after.next_staking_round.total_stake,
            cortex_account_before.next_staking_round.total_stake + params.amount
        );
    }

    // check `Stake` data update
    {
        let stake_account_after = utils::get_account::<Stake>(program_test_ctx, stake_pda).await;
        // conditionnal checks if the account was initialized previously
        if let Some(s) = stake_account_before {
            assert_eq!(stake_account_after.amount, s.amount + params.amount);

            // Note - there is a claiming part that isn't tested here that can be added once we have the
            // duration of a staking_round
        } else {
            assert_eq!(stake_account_after.amount, params.amount);
        }
        assert_eq!(stake_account_after.bump, stake_bump);

        let clock = program_test_ctx.banks_client.get_sysvar::<Clock>().await?;
        assert_eq!(stake_account_after.stake_time, clock.unix_timestamp);
    }

    Ok(())
}
