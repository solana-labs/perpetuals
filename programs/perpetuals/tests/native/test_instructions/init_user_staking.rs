use {
    crate::utils::{self, pda},
    anchor_lang::{AnchorSerialize, ToAccountMetas},
    perpetuals::{
        instructions::InitUserStakingParams,
        state::{
            staking::Staking,
            user_staking::{UserStaking, CLOCKWORK_PAYER_PUBKEY},
        },
    },
    solana_program::pubkey::Pubkey,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    std::str::FromStr,
    tokio::sync::RwLock,
};

pub async fn init_user_staking(
    program_test_ctx: &RwLock<ProgramTestContext>,
    owner: &Keypair,
    payer: &Keypair,
    staked_token_mint: &Pubkey,
    params: InitUserStakingParams,
) -> std::result::Result<(Pubkey, u8), BanksClientError> {
    // ==== GIVEN =============================================================
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda: Pubkey = pda::get_cortex_pda().0;
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
    let staking_pda = pda::get_staking_pda(staked_token_mint).0;

    let staking_account = utils::get_account::<Staking>(program_test_ctx, staking_pda).await;

    let (user_staking_pda, user_staking_bump) =
        pda::get_user_staking_pda(&owner.pubkey(), &staking_pda);
    let user_staking_thread_authority_pda =
        pda::get_user_staking_thread_authority(&user_staking_pda).0;
    let staking_reward_token_vault_pda = pda::get_staking_reward_token_vault_pda(&staking_pda).0;
    let staking_lm_reward_token_vault_pda =
        pda::get_staking_lm_reward_token_vault_pda(&staking_pda).0;
    let reward_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &staking_account.reward_token_mint).0;
    let lm_token_account_address =
        utils::find_associated_token_account(&owner.pubkey(), &lm_token_mint_pda).0;

    let stakes_claim_cron_thread_address = pda::get_thread_address(
        &user_staking_thread_authority_pda,
        params.stakes_claim_cron_thread_id.try_to_vec().unwrap(),
    );

    // ==== WHEN ==============================================================

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        perpetuals::accounts::InitUserStaking {
            owner: owner.pubkey(),
            reward_token_account: reward_token_account_address,
            lm_token_account: lm_token_account_address,
            staking_reward_token_vault: staking_reward_token_vault_pda,
            staking_lm_reward_token_vault: staking_lm_reward_token_vault_pda,
            user_staking: user_staking_pda,
            staking: staking_pda,
            transfer_authority: transfer_authority_pda,
            user_staking_thread_authority: user_staking_thread_authority_pda,
            stakes_claim_cron_thread: stakes_claim_cron_thread_address,
            stakes_claim_payer: Pubkey::from_str(CLOCKWORK_PAYER_PUBKEY).unwrap(),
            cortex: cortex_pda,
            perpetuals: perpetuals_pda,
            lm_token_mint: lm_token_mint_pda,
            staking_reward_token_mint: staking_account.reward_token_mint,
            perpetuals_program: perpetuals::ID,
            clockwork_program: clockwork_sdk::ID,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        perpetuals::instruction::InitUserStaking {
            params: InitUserStakingParams {
                stakes_claim_cron_thread_id: params.stakes_claim_cron_thread_id,
            },
        },
        Some(&payer.pubkey()),
        &[owner, payer],
        None,
        None,
    )
    .await?;

    // ==== THEN ==============================================================
    let user_staking_account =
        utils::get_account::<UserStaking>(program_test_ctx, user_staking_pda).await;

    {
        assert_eq!(user_staking_account.bump, user_staking_bump);
        assert_eq!(user_staking_account.locked_stakes.len(), 0);
        assert_eq!(user_staking_account.liquid_stake.amount, 0);
    }

    Ok((user_staking_pda, user_staking_bump))
}
