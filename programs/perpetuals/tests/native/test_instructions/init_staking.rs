use {
    crate::utils::{self, pda},
    anchor_lang::{
        prelude::{AccountMeta, Pubkey},
        ToAccountMetas,
    },
    perpetuals::{instructions::InitStakingParams, state::multisig::Multisig},
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::{keypair::Keypair, Signer},
    tokio::sync::RwLock,
};

pub async fn init_staking(
    program_test_ctx: &RwLock<ProgramTestContext>,
    // Admin must be a part of the multisig
    admin: &Keypair,
    payer: &Keypair,
    staking_reward_token_mint: &Pubkey,
    staked_token_mint: &Pubkey,
    params: &InitStakingParams,
    multisig_signers: &[&Keypair],
) -> std::result::Result<(Pubkey, u8), BanksClientError> {
    // ==== WHEN ==============================================================
    let multisig_pda = pda::get_multisig_pda().0;
    let transfer_authority_pda = pda::get_transfer_authority_pda().0;
    let perpetuals_pda = pda::get_perpetuals_pda().0;
    let cortex_pda = pda::get_cortex_pda().0;
    let (staking_pda, staking_bump) = pda::get_staking_pda(staked_token_mint);
    let lm_token_mint_pda = pda::get_lm_token_mint_pda().0;
    let staking_staked_token_vault_pda = pda::get_staking_staked_token_vault_pda(&staking_pda).0;
    let staking_reward_token_vault_pda = pda::get_staking_reward_token_vault_pda(&staking_pda).0;
    let staking_lm_reward_token_vault_pda =
        pda::get_staking_lm_reward_token_vault_pda(&staking_pda).0;

    let multisig_account = utils::get_account::<Multisig>(program_test_ctx, multisig_pda).await;

    // One Tx per multisig signer
    for i in 0..multisig_account.min_signatures {
        let signer: &Keypair = multisig_signers[i as usize];

        let accounts_meta = {
            let accounts = perpetuals::accounts::InitStaking {
                admin: admin.pubkey(),
                payer: payer.pubkey(),
                multisig: multisig_pda,
                transfer_authority: transfer_authority_pda,
                staking: staking_pda,
                lm_token_mint: lm_token_mint_pda,
                cortex: cortex_pda,
                perpetuals: perpetuals_pda,
                staking_staked_token_vault: staking_staked_token_vault_pda,
                staking_reward_token_vault: staking_reward_token_vault_pda,
                staking_lm_reward_token_vault: staking_lm_reward_token_vault_pda,
                staking_reward_token_mint: *staking_reward_token_mint,
                staking_staked_token_mint: *staked_token_mint,
                system_program: anchor_lang::system_program::ID,
                token_program: anchor_spl::token::ID,
                rent: solana_program::sysvar::rent::ID,
            };

            let mut accounts_meta = accounts.to_account_metas(None);

            accounts_meta.push(AccountMeta {
                pubkey: signer.pubkey(),
                is_signer: true,
                is_writable: false,
            });

            accounts_meta
        };

        utils::create_and_execute_perpetuals_ix(
            program_test_ctx,
            accounts_meta,
            perpetuals::instruction::InitStaking {
                params: InitStakingParams {
                    staking_type: params.staking_type,
                },
            },
            Some(&payer.pubkey()),
            &[admin, payer, signer],
            None,
            None,
        )
        .await?;
    }

    // ==== THEN ==============================================================
    // TODO, check the created staking account

    Ok((staking_pda, staking_bump))
}
