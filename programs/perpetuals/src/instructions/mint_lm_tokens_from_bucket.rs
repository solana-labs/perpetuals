//! MintLmTokensFromBucket instruction handler

use {
    crate::{
        error::PerpetualsError,
        math,
        state::{
            cortex::Cortex,
            multisig::{AdminInstruction, Multisig},
            perpetuals::Perpetuals,
        },
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
};

#[derive(Accounts)]
pub struct MintLmTokensFromBucket<'info> {
    #[account()]
    pub admin: Signer<'info>,

    #[account(
        mut,
        constraint = receiving_account.mint == lm_token_mint.key(),
    )]
    pub receiving_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"cortex"],
        bump = cortex.bump,
    )]
    pub cortex: Box<Account<'info, Cortex>>,

    #[account(
        seeds = [b"perpetuals"],
        bump = perpetuals.perpetuals_bump
    )]
    pub perpetuals: Box<Account<'info, Perpetuals>>,

    #[account(
        mut,
        seeds = [b"lm_token_mint"],
        bump = cortex.lm_token_bump
    )]
    pub lm_token_mint: Box<Account<'info, Mint>>,

    token_program: Program<'info, Token>,
    //
    //
    // Remaining account if called from outside (not cpi)
    // "multisig"
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct MintLmTokensFromBucketParams {
    pub bucket_name: BucketName,
    pub amount: u64,
    pub reason: String,
}

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone)]
pub enum BucketName {
    CoreContributor,
    DaoTreasury,
    PoL,
    Ecosystem,
}

pub fn mint_lm_tokens_from_bucket<'info>(
    ctx: Context<'_, '_, '_, 'info, MintLmTokensFromBucket<'info>>,
    params: &MintLmTokensFromBucketParams,
) -> Result<u8> {
    {
        msg!("Validate inputs");

        if params.amount == 0 {
            return Err(ProgramError::InvalidArgument.into());
        }
    }

    // Check authentication, accept either transfer_authority as admin, or multisig signatures
    {
        if ctx.accounts.admin.key() == ctx.accounts.transfer_authority.key() {
            // Good
        } else {
            // Load multisig account from remaining_accounts
            let mut multisig: Multisig = {
                let accounts_iter = &mut ctx.remaining_accounts.iter();
                let account_info = next_account_info(accounts_iter)?;
                let data = account_info.data.borrow_mut();
                let data_slice: &mut &[u8] = &mut (data.as_ref());

                let multisig = Multisig::try_deserialize_unchecked(data_slice)?;

                require_eq!(
                    account_info.key(),
                    Pubkey::find_program_address(&[b"multisig"], &crate::ID).0,
                );

                multisig
            };

            {
                let signatures_left = multisig.sign_multisig(
                    &ctx.accounts.admin,
                    &Multisig::get_account_infos(&ctx)[1..],
                    &Multisig::get_instruction_data(
                        AdminInstruction::MintLmTokensFromBucket,
                        params,
                    )?,
                )?;

                if signatures_left > 0 {
                    msg!(
                        "Instruction has been signed but more signatures are required: {}",
                        signatures_left
                    );
                    return Ok(signatures_left);
                }
            }
        }
    }

    let cortex = ctx.accounts.cortex.as_mut();

    msg!(
        "Mint {} LM tokens for {} bucket",
        params.amount,
        match params.bucket_name {
            BucketName::CoreContributor => "core_contributor",
            BucketName::DaoTreasury => "dao_treasury",
            BucketName::PoL => "pol",
            BucketName::Ecosystem => "ecosystem",
        }
    );

    msg!("Reason: {}", params.reason);

    match params.bucket_name {
        BucketName::CoreContributor => {
            cortex.core_contributor_bucket_minted_amount =
                math::checked_add(cortex.core_contributor_bucket_minted_amount, params.amount)?;

            require!(
                cortex.core_contributor_bucket_minted_amount
                    <= cortex.core_contributor_bucket_allocation,
                PerpetualsError::BucketMintLimit
            );
        }
        BucketName::DaoTreasury => {
            cortex.dao_treasury_bucket_minted_amount =
                math::checked_add(cortex.dao_treasury_bucket_minted_amount, params.amount)?;

            require!(
                cortex.dao_treasury_bucket_minted_amount <= cortex.dao_treasury_bucket_allocation,
                PerpetualsError::BucketMintLimit
            );
        }
        BucketName::PoL => {
            cortex.pol_bucket_minted_amount =
                math::checked_add(cortex.pol_bucket_minted_amount, params.amount)?;

            require!(
                cortex.pol_bucket_minted_amount <= cortex.pol_bucket_allocation,
                PerpetualsError::BucketMintLimit
            );
        }
        BucketName::Ecosystem => {
            cortex.ecosystem_bucket_minted_amount =
                math::checked_add(cortex.ecosystem_bucket_minted_amount, params.amount)?;

            require!(
                cortex.ecosystem_bucket_minted_amount <= cortex.ecosystem_bucket_allocation,
                PerpetualsError::BucketMintLimit
            );
        }
    }

    ctx.accounts.perpetuals.mint_tokens(
        ctx.accounts.lm_token_mint.to_account_info(),
        ctx.accounts.receiving_account.to_account_info(),
        ctx.accounts.transfer_authority.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        params.amount,
    )?;

    msg!(
        "core_contributor bucket: {}/{}",
        cortex.core_contributor_bucket_minted_amount,
        cortex.core_contributor_bucket_allocation
    );

    msg!(
        "dao_treasury bucket: {}/{}",
        cortex.dao_treasury_bucket_minted_amount,
        cortex.dao_treasury_bucket_allocation
    );

    msg!(
        "pol bucket: {}/{}",
        cortex.pol_bucket_minted_amount,
        cortex.pol_bucket_allocation
    );

    msg!(
        "ecosystem bucket: {}/{}",
        cortex.ecosystem_bucket_minted_amount,
        cortex.ecosystem_bucket_allocation
    );

    Ok(0)
}
