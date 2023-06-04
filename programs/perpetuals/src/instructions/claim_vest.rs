use {
    crate::{
        adapters::SplGovernanceV3Adapter,
        error::PerpetualsError,
        math,
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

// Return claimed amount
pub fn claim_vest<'info>(ctx: Context<'_, '_, '_, 'info, ClaimVest<'info>>) -> Result<u64> {
    // validate owner
    require!(
        ctx.accounts.vest.owner == ctx.accounts.owner.key(),
        PerpetualsError::InvalidVestState
    );

    let vest = ctx.accounts.vest.as_mut();

    let current_time = ctx.accounts.perpetuals.get_time()?;

    let claimable_amount = vest.get_claimable_amount(current_time)?;

    if claimable_amount == 0 {
        return Ok(0);
    }

    // Mint lm token to user account
    {
        ctx.accounts.perpetuals.mint_tokens(
            ctx.accounts.lm_token_mint.to_account_info(),
            ctx.accounts.receiving_account.to_account_info(),
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            claimable_amount,
        )?;
    }

    // Update vest accounting
    {
        vest.claimed_amount = math::checked_add(vest.claimed_amount, claimable_amount)?;
        vest.last_claim_timestamp = current_time;
    }

    // If everything have been claimed, remove vesting from the cortex list
    if vest.claimed_amount == vest.amount {
        let cortex = ctx.accounts.cortex.as_mut();

        let vest_idx = cortex
            .vests
            .iter()
            .position(|x| *x == ctx.accounts.vest.key())
            .ok_or(PerpetualsError::InvalidVestState)?;

        cortex.vests.remove(vest_idx);
    }

    // Revoke 1:1 governing power for each claimed tokens
    {
        let perpetuals = ctx.accounts.perpetuals.as_mut();
        perpetuals.remove_governing_power(
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.owner.to_account_info(),
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
            claimable_amount,
        )?;
    }

    // Note: the vest PDA still lives, we can unalloc (currently works same as Pool, without removal)

    Ok(claimable_amount)
}
