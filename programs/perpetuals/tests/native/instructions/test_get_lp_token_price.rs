use {
    crate::utils::{self, pda},
    anchor_lang::{prelude::Pubkey, ToAccountMetas},
    perpetuals::{
        instructions::GetLpTokenPriceParams,
        state::{custody::Custody, pool::Pool},
    },
    solana_program::instruction::AccountMeta,
    solana_program_test::{BanksClientError, ProgramTestContext},
    solana_sdk::signer::keypair::Keypair,
    tokio::sync::RwLock,
};

#[allow(clippy::too_many_arguments)]
pub async fn test_get_lp_token_price(
    program_test_ctx: &RwLock<ProgramTestContext>,
    payer: &Keypair,
    pool_pda: &Pubkey,
    lp_token_mint_pda: &Pubkey,
) -> std::result::Result<u64, BanksClientError> {
    // ==== WHEN ==============================================================
    let perpetuals_pda = pda::get_perpetuals_pda().0;

    let accounts_meta = {
        let accounts = perpetuals::accounts::GetLpTokenPrice {
            perpetuals: perpetuals_pda,
            pool: *pool_pda,
            lp_token_mint: *lp_token_mint_pda,
        };

        let mut accounts_meta = accounts.to_account_metas(None);

        let pool_account = utils::get_account::<Pool>(program_test_ctx, *pool_pda).await;

        // For each token, add custody account as remaining_account
        for custody in &pool_account.custodies {
            accounts_meta.push(AccountMeta {
                pubkey: *custody,
                is_signer: false,
                is_writable: false,
            });
        }

        // For each token, add custody oracle account as remaining_account
        for custody in &pool_account.custodies {
            let custody_account = utils::get_account::<Custody>(program_test_ctx, *custody).await;

            accounts_meta.push(AccountMeta {
                pubkey: custody_account.oracle.oracle_account,
                is_signer: false,
                is_writable: false,
            });
        }

        accounts_meta
    };

    let result: u64 = utils::create_and_simulate_perpetuals_view_ix(
        program_test_ctx,
        accounts_meta,
        perpetuals::instruction::GetLpTokenPrice {
            params: GetLpTokenPriceParams {},
        },
        payer,
    )
    .await?;

    // ==== THEN ==============================================================
    Ok(result)
}
