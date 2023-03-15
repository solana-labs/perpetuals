use {
    crate::utils::{self, pda},
    anchor_lang::ToAccountMetas,
    bonfida_test_utils::ProgramTestContextExt,
    perpetuals::{
        instructions::AddStakeParams,
        state::{cortex::Cortex, stake::Stake},
    },
    solana_program_test::BanksClientError,
    solana_program_test::ProgramTestContext,
    solana_sdk::signer::{keypair::Keypair, Signer},
};

pub async fn test_add_stake(
    program_test_ctx: &mut ProgramTestContext,
    owner: &Keypair,
    payer: &Keypair,
    params: AddStakeParams,
) -> std::result::Result<(), BanksClientError> {
    // ==== GIVEN =============================================================
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let (stake_pda, stake_bump) = pda::get_stake_pda(&owner.pubkey());
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let stake_token_account_pda = pda::get_stake_token_account_pda().0;
    let stake_redeemable_token_mint_pda = pda::get_stake_redeemable_token_mint_pda().0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;

    let lm_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lm_token_mint_pda).0;
    let redeemable_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &stake_redeemable_token_mint_pda).0;

    // // ==== WHEN ==============================================================
    // save account state before tx execution
    let cortex_acount_before = utils::get_account::<Cortex>(program_test_ctx, cortex_pda).await;
    let stake_acount_before = utils::try_get_account::<Stake>(program_test_ctx, stake_pda).await;
    let owner_lm_token_account_before = program_test_ctx
        .get_token_account(lm_token_account_address)
        .await
        .unwrap();
    let owner_redeemable_token_account_before = program_test_ctx
        .get_token_account(redeemable_token_account_address)
        .await
        .unwrap();

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::AddStake {
            owner: owner.pubkey(),
            funding_account: lm_token_account_address,
            redeemable_token_account: redeemable_token_account_address,
            stake_token_account: stake_token_account_pda,
            transfer_authority: transfer_authority_pda,
            stake: stake_pda,
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            lm_token_mint: lm_token_mint_pda,
            stake_redeemable_token_mint: stake_redeemable_token_mint_pda,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::Stake {
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
        let owner_redeemable_token_account_after = program_test_ctx
            .get_token_account(redeemable_token_account_address)
            .await
            .unwrap();

        assert_eq!(
            owner_lm_token_account_before.amount,
            owner_lm_token_account_after.amount + params.amount
        );
        assert!(
            owner_redeemable_token_account_before.amount
                < owner_redeemable_token_account_after.amount
        );
    }

    // check `Cortex` data update
    {
        let cortex_acount_after = utils::get_account::<Cortex>(program_test_ctx, cortex_pda).await;
        let total_stake_before = cortex_acount_before
            .staking_rounds
            .last()
            .unwrap()
            .total_stake;
        let total_stake_after = cortex_acount_after
            .staking_rounds
            .last()
            .unwrap()
            .total_stake;
        assert_eq!(total_stake_after, total_stake_before + params.amount);
    }

    // check `Stake` data update
    {
        let stake_acount_after = utils::get_account::<Stake>(program_test_ctx, stake_pda).await;
        // conditionnal checks if the account was initialized previously
        if let Some(s) = stake_acount_before {
            assert_eq!(stake_acount_after.amount, s.amount + params.amount);

            // Note - there is a claiming part that isn't tested here that can be added once we have the
            // duration of a staking_round
        }
        assert_eq!(stake_acount_after.bump, stake_bump);
        assert_ne!(stake_acount_after.inception_time, 0);
    }

    Ok(())
}
