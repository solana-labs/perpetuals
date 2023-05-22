use {
    crate::{
        adapters::SplGovernanceV3Adapter,
        error::PerpetualsError,
        governance::remove_governing_power,
        state::{cortex::Cortex, perpetuals::Perpetuals, vest::Vest},
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
        bump = cortex.bump,
        has_one = governance_program @PerpetualsError::InvalidGovernanceProgram,
        has_one = governance_realm @PerpetualsError::InvalidGovernanceRealm,
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
        bump = cortex.lm_token_bump
    )]
    pub lm_token_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        seeds = [b"governance_token_mint"],
        bump = cortex.governance_token_bump
    )]
    pub governance_token_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        seeds = [
            b"vest_token_account",
            vest.key().as_ref(),
        ],
        token::authority = transfer_authority,
        token::mint = lm_token_mint,
        bump = vest.vest_token_account_bump
    )]
    pub vest_token_account: Box<Account<'info, TokenAccount>>,

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

    // Transfer vested token to user account
    {
        let perpetuals = ctx.accounts.perpetuals.as_ref();

        perpetuals.transfer_tokens(
            ctx.accounts.vest_token_account.to_account_info(),
            ctx.accounts.receiving_account.to_account_info(),
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.vest.amount,
        )?;
    }

    // Remove governing power from Vest account (and revoke delegation to owner)
    {
        let authority_seeds: &[&[u8]] = &[
            b"transfer_authority",
            &[ctx.accounts.perpetuals.transfer_authority_bump],
        ];
        let vest_seeds: &[&[u8]] = &[
            b"vest",
            ctx.accounts.owner.key.as_ref(),
            &[ctx.accounts.vest.bump],
        ];

        let amount = ctx.accounts.vest.amount;
        let owner = &ctx.accounts.owner;
        msg!(
            "Governance - Burn {} governing token to Vest account, and revoke them from the owner: {}",
            amount,
            owner.key
        );
        remove_governing_power(
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.vest.to_account_info(),
            ctx.accounts
                .governance_governing_token_owner_record
                .to_account_info(),
            ctx.accounts.governance_token_mint.to_account_info(),
            ctx.accounts.governance_realm.to_account_info(),
            ctx.accounts.governance_realm_config.to_account_info(),
            ctx.accounts
                .governance_governing_token_holding
                .to_account_info(),
            ctx.accounts.governance_program.to_account_info(),
            authority_seeds,
            vest_seeds,
            amount,
            owner.to_account_info(),
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
