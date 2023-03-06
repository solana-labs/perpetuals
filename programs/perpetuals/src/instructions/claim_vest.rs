use {
    crate::state::{cortex::Cortex, perpetuals::Perpetuals, vest::Vest},
    crate::{
        adapters::{self, SplGovernanceV3Adapter},
        error::PerpetualsError,
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
};

#[derive(Accounts)]
#[instruction()]
pub struct ClaimVest<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        constraint = receiving_account.mint == lm_token_mint.key(),
        has_one = owner
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
        realloc = Cortex::LEN + (cortex.vests.len() + 1) * std::mem::size_of::<Vest>(),
        realloc::payer = owner,
        realloc::zero = false,
        seeds = [b"cortex"],
        bump = cortex.bump
    )]
    pub cortex: Box<Account<'info, Cortex>>,

    #[account(
        mut,
        seeds = [b"perpetuals"],
        bump = perpetuals.perpetuals_bump
    )]
    pub perpetuals: Box<Account<'info, Perpetuals>>,

    #[account(
        mut,
        seeds = [b"vest", owner.key().as_ref()],
        bump
    )]
    pub vest: Box<Account<'info, Vest>>,

    #[account(
        mut,
        seeds = [b"lm_token_mint"],
        bump
    )]
    pub lm_token_mint: Box<Account<'info, Mint>>,

    /// CHECK: checked by spl governance v3 program
    /// A realm represent one project (ADRENA, MANGO etc.) within the governance program
    pub governance_realm: UncheckedAccount<'info>,

    /// CHECK: checked by spl governance v3 program
    pub governance_realm_config: UncheckedAccount<'info>,

    /// CHECK: checked by spl governance v3 program
    /// Token account owned by governance program holding user's locked tokens
    #[account(mut)]
    pub governance_governing_token_holding: UncheckedAccount<'info>,

    /// CHECK: checked by spl governance v3 program
    /// Account owned by governance storing user informations
    #[account(mut)]
    pub governance_governing_token_owner_record: UncheckedAccount<'info>,

    pub governance_program: Program<'info, SplGovernanceV3Adapter>,

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
    rent: Sysvar<'info, Rent>,
}

pub fn claim_vest<'info>(ctx: Context<'_, '_, '_, 'info, ClaimVest<'info>>) -> Result<u8> {
    {
        // validate owner
        require!(
            ctx.accounts.vest.owner == ctx.accounts.owner.key(),
            PerpetualsError::InvalidVestState
        );

        // validate maturation of vest
        require!(
            ctx.accounts
                .vest
                .is_claimable(ctx.accounts.lm_token_mint.supply)?,
            PerpetualsError::InvalidVestState
        );
    }

    // Revoke vote delegation
    {
        let owner_key = ctx.accounts.owner.key();
        let vest_signer_seeds: &[&[u8]] = &[b"vest", owner_key.as_ref(), &[ctx.accounts.vest.bump]];

        let cpi_accounts = adapters::SetGovernanceDelegate {
            realm: ctx.accounts.governance_realm.to_account_info(),
            governance_authority: ctx.accounts.vest.to_account_info(),
            governing_token_mint: ctx.accounts.lm_token_mint.to_account_info(),
            governing_token_owner: ctx.accounts.vest.to_account_info(),
            governing_token_owner_record: ctx
                .accounts
                .governance_governing_token_owner_record
                .to_account_info(),
        };

        let cpi_program = ctx.accounts.governance_program.to_account_info();

        adapters::set_governance_delegate(
            CpiContext::new(cpi_program, cpi_accounts).with_signer(&[vest_signer_seeds]),
        )?;
    }

    // Withdraw tokens from governance directly to the vest owner token account
    {
        let owner_key = ctx.accounts.owner.key();
        let vest_signer_seeds: &[&[u8]] = &[b"vest", owner_key.as_ref(), &[ctx.accounts.vest.bump]];

        let cpi_accounts = adapters::WithdrawGoverningTokens {
            realm: ctx.accounts.governance_realm.to_account_info(),
            realm_config: ctx.accounts.governance_realm_config.to_account_info(),
            governing_token_mint: ctx.accounts.lm_token_mint.to_account_info(),
            governing_token_destination: ctx.accounts.receiving_account.to_account_info(),
            governing_token_owner: ctx.accounts.vest.to_account_info(),
            governing_token_holding: ctx
                .accounts
                .governance_governing_token_holding
                .to_account_info(),
            governing_token_owner_record: ctx
                .accounts
                .governance_governing_token_owner_record
                .to_account_info(),
        };

        let cpi_program = ctx.accounts.governance_program.to_account_info();

        adapters::withdraw_governing_tokens(
            CpiContext::new(cpi_program, cpi_accounts).with_signer(&[vest_signer_seeds]),
        )?;
    }

    // remove vest from the list
    {
        let cortex = ctx.accounts.cortex.as_mut();

        let vest_idx = cortex
            .vests
            .iter()
            .position(|x| *x == ctx.accounts.vest.key())
            .ok_or(PerpetualsError::InvalidVestState)?;

        cortex.vests.remove(vest_idx);
    }

    // Note: the vest PDA still lives, we can unalloc (currently works same as Pool, without removal)

    Ok(0)
}
