use {
    crate::utils::{self, pda},
    anchor_lang::{
        prelude::{AccountMeta, Pubkey},
        ToAccountMetas,
    },
    perpetuals::{
        adapters::spl_governance_program_adapter,
        instructions::InitParams,
        state::{
            cortex::Cortex,
            multisig::Multisig,
            perpetuals::Perpetuals,
            staking::{Staking, StakingRound},
        },
    },
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    tokio::sync::RwLock,
};

pub async fn init(
    program_test_ctx: &RwLock<ProgramTestContext>,
    upgrade_authority: &Keypair,
    params: InitParams,
    governance_realm_pda: &Pubkey,
    staking_reward_token_mint: &Pubkey,
    multisig_signers: &[&Keypair],
) -> std::result::Result<(), BanksClientError> {
    // ==== WHEN ==============================================================
    let perpetuals_program_data_pda = pda::get_program_data_pda().0;
    let (multisig_pda, multisig_bump) = pda::get_multisig_pda();
    let (lm_token_mint_pda, lm_token_mint_bump) = pda::get_lm_token_mint_pda();
    let (lm_staking_pda, lm_staking_bump) = pda::get_staking_pda(&lm_token_mint_pda);
    let (transfer_authority_pda, transfer_authority_bump) = pda::get_transfer_authority_pda();
    let (perpetuals_pda, perpetuals_bump) = pda::get_perpetuals_pda();
    let (cortex_pda, cortex_bump) = pda::get_cortex_pda();
    let (governance_token_mint_pda, governance_token_mint_bump) =
        pda::get_governance_token_mint_pda();
    let (staking_staked_token_vault_pda, staking_token_account_bump) =
        pda::get_staking_staked_token_vault_pda(&lm_staking_pda);
    let (staking_reward_token_vault_pda, staking_reward_token_account_bump) =
        pda::get_staking_reward_token_vault_pda(&lm_staking_pda);
    let (staking_lm_reward_token_vault_pda, staking_lm_reward_token_account_bump) =
        pda::get_staking_lm_reward_token_vault_pda(&lm_staking_pda);

    let accounts_meta = {
        let accounts = perpetuals::accounts::Init {
            upgrade_authority: upgrade_authority.pubkey(),
            multisig: multisig_pda,
            transfer_authority: transfer_authority_pda,
            lm_staking: lm_staking_pda,
            cortex: cortex_pda,
            lm_token_mint: lm_token_mint_pda,
            governance_token_mint: governance_token_mint_pda,
            lm_staking_staked_token_vault: staking_staked_token_vault_pda,
            lm_staking_reward_token_vault: staking_reward_token_vault_pda,
            lm_staking_lm_reward_token_vault: staking_lm_reward_token_vault_pda,
            perpetuals: perpetuals_pda,
            perpetuals_program: perpetuals::ID,
            perpetuals_program_data: perpetuals_program_data_pda,
            governance_realm: *governance_realm_pda,
            governance_program: spl_governance_program_adapter::ID,
            lm_staking_reward_token_mint: *staking_reward_token_mint,
            system_program: anchor_lang::system_program::ID,
            token_program: anchor_spl::token::ID,
            rent: solana_program::sysvar::rent::ID,
        };

        let mut accounts_meta = accounts.to_account_metas(None);

        for signer in multisig_signers {
            accounts_meta.push(AccountMeta {
                pubkey: signer.pubkey(),
                is_signer: true,
                is_writable: false,
            });
        }

        accounts_meta
    };

    utils::create_and_execute_perpetuals_ix(
        program_test_ctx,
        accounts_meta,
        perpetuals::instruction::Init { params },
        Some(&upgrade_authority.pubkey()),
        &[&[upgrade_authority], multisig_signers].concat(),
        None,
        None,
    )
    .await?;

    // ==== THEN ==============================================================
    let perpetuals_account =
        utils::get_account::<Perpetuals>(program_test_ctx, perpetuals_pda).await;

    // Assert permissions
    {
        let p = perpetuals_account.permissions;

        assert_eq!(p.allow_swap, params.allow_swap);
        assert_eq!(p.allow_add_liquidity, params.allow_add_liquidity);
        assert_eq!(p.allow_remove_liquidity, params.allow_remove_liquidity);
        assert_eq!(p.allow_open_position, params.allow_open_position);
        assert_eq!(p.allow_close_position, params.allow_close_position);
        assert_eq!(p.allow_pnl_withdrawal, params.allow_pnl_withdrawal);
        assert_eq!(
            p.allow_collateral_withdrawal,
            params.allow_collateral_withdrawal
        );
        assert_eq!(p.allow_size_change, params.allow_size_change);
    }

    assert_eq!(
        perpetuals_account.transfer_authority_bump,
        transfer_authority_bump
    );
    assert_eq!(perpetuals_account.perpetuals_bump, perpetuals_bump);

    let cortex_account = utils::get_account::<Cortex>(program_test_ctx, cortex_pda).await;
    // Assert cortex
    {
        assert_eq!(cortex_account.bump, cortex_bump);
        assert_eq!(cortex_account.lm_token_bump, lm_token_mint_bump);
        assert_eq!(
            cortex_account.governance_token_bump,
            governance_token_mint_bump
        );
        assert_eq!(cortex_account.inception_epoch, 0);
    }

    let multisig_account = utils::get_account::<Multisig>(program_test_ctx, multisig_pda).await;
    // Assert multisig
    {
        assert_eq!(multisig_account.bump, multisig_bump);
        assert_eq!(multisig_account.min_signatures, params.min_signatures);

        // Check signers
        {
            for (i, signer) in multisig_signers.iter().enumerate() {
                assert_eq!(multisig_account.signers[i], signer.pubkey());
            }
        }
    }

    let staking_account = utils::get_account::<Staking>(program_test_ctx, lm_staking_pda).await;
    // Assert staking account
    {
        assert_eq!(staking_account.bump, lm_staking_bump);

        assert_eq!(
            staking_account.staked_token_vault_bump,
            staking_token_account_bump
        );
        assert_eq!(
            staking_account.reward_token_vault_bump,
            staking_reward_token_account_bump
        );
        assert_eq!(
            staking_account.lm_reward_token_vault_bump,
            staking_lm_reward_token_account_bump
        );

        assert_eq!(staking_account.resolved_reward_token_amount, u64::MIN);
        assert_eq!(staking_account.resolved_staked_token_amount, u64::MIN);
        assert_eq!(staking_account.resolved_lm_reward_token_amount, u64::MIN);
        assert_eq!(staking_account.resolved_lm_staked_token_amount, u64::MIN);
        assert_eq!(staking_account.next_staking_round, StakingRound::new(0));
        assert!(staking_account.resolved_staking_rounds.is_empty());
    }

    Ok(())
}
