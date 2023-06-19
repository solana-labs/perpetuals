use {
    crate::utils::{self, pda},
    anchor_lang::{AnchorSerialize, ToAccountMetas},
    perpetuals::{
        instructions::InitStakingParams,
        state::staking::{Staking, CLOCKWORK_PAYER_PUBKEY},
    },
    solana_program::pubkey::Pubkey,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    std::str::FromStr,
};

pub async fn test_init_staking(
    program_test_ctx: &mut ProgramTestContext,
    owner: &Keypair,
    payer: &Keypair,
    stake_reward_token_mint: &Pubkey,
    params: InitStakingParams,
) -> std::result::Result<(Pubkey, u8), BanksClientError> {
    // ==== GIVEN =============================================================
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let (staking_pda, staking_bump) = pda::get_staking_pda(&owner.pubkey());
    let staking_thread_authority_pda = pda::get_staking_thread_authority(&owner.pubkey()).0;
    let stake_reward_token_account_pda = pda::get_stake_reward_token_account_pda().0;
    let stake_reward_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), stake_reward_token_mint).0;

    let stakes_claim_cron_thread_address = pda::get_thread_address(
        &staking_thread_authority_pda,
        params.stakes_claim_cron_thread_id.try_to_vec().unwrap(),
    );

    // ==== WHEN ==============================================================

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::InitStaking {
            owner: owner.pubkey(),
            owner_reward_token_account: stake_reward_token_account_address,
            stake_reward_token_account: stake_reward_token_account_pda,
            staking: staking_pda,
            transfer_authority: transfer_authority_pda,
            staking_thread_authority: staking_thread_authority_pda,
            stakes_claim_cron_thread: stakes_claim_cron_thread_address,
            stakes_claim_payer: Pubkey::from_str(CLOCKWORK_PAYER_PUBKEY).unwrap(),
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            stake_reward_token_mint: *stake_reward_token_mint,
            perpetuals_program: perpetuals::ID,
            clockwork_program: clockwork_sdk::ID,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::InitStaking {
            params: InitStakingParams {
                stakes_claim_cron_thread_id: params.stakes_claim_cron_thread_id,
            },
        },
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
