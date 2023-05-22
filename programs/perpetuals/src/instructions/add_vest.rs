//! AddVest instruction handler

use {
    crate::{
        adapters::SplGovernanceV3Adapter,
        error::PerpetualsError,
        governance::add_governing_power,
        state::{
            cortex::Cortex,
            multisig::{AdminInstruction, Multisig},
            perpetuals::Perpetuals,
            vest::Vest,
        },
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
};

#[derive(Accounts)]
#[instruction(params: AddVestParams)]
pub struct AddVest<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: can be any wallet
    #[account()]
    pub owner: AccountInfo<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [b"multisig"],
        bump = multisig.load()?.bump
    )]
    pub multisig: AccountLoader<'info, Multisig>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    #[account(
        mut,
        realloc = cortex.size() + std::mem::size_of::<Vest>(),
        realloc::payer = admin,
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

    // instruction can be called multiple times due to multisig use, hence init_if_needed
    // instead of init. On the first call account is zero initialized and filled out when
    // all signatures are collected. When account is in zeroed state it can't be used in other
    // instructions because seeds are computed with the pool name. Uniqueness is enforced
    // manually in the instruction handler.
    #[account(
        init_if_needed,
        payer = admin,
        space = Vest::LEN,
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
        init_if_needed,
        seeds = [
            b"vest_token_account",
            vest.key().as_ref(),
        ],
        token::authority = transfer_authority,
        token::mint = lm_token_mint,
        bump,
        payer = payer,
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

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddVestParams {
    pub amount: u64,
    pub unlock_share: u64,
}

pub fn add_vest<'info>(
    ctx: Context<'_, '_, '_, 'info, AddVest<'info>>,
    params: &AddVestParams,
) -> Result<u8> {
    // validate inputs
    if params.amount == 0 || params.unlock_share == 0 {
        return Err(ProgramError::InvalidArgument.into());
    }

    // validate signatures
    {
        let mut multisig = ctx.accounts.multisig.load_mut()?;

        let signatures_left = multisig.sign_multisig(
            &ctx.accounts.admin,
            &Multisig::get_account_infos(&ctx)[1..],
            &Multisig::get_instruction_data(AdminInstruction::AddPool, params)?,
        )?;

        if signatures_left > 0 {
            msg!(
                "Instruction has been signed but more signatures are required: {}",
                signatures_left
            );
            return Ok(signatures_left);
        }
    }

    // setup vest account
    {
        let vest = ctx.accounts.vest.as_mut();

        if vest.inception_time != 0 {
            // return error if pool is already initialized
            return Err(ProgramError::AccountAlreadyInitialized.into());
        }

        msg!(
            "Record vest: share {} BPS, owner {}",
            params.unlock_share,
            ctx.accounts.owner.key
        );

        vest.amount = params.amount;
        vest.unlock_share = params.unlock_share;
        vest.owner = ctx.accounts.owner.key();
        vest.bump = *ctx.bumps.get("vest").ok_or(ProgramError::InvalidSeeds)?;
        vest.inception_time = ctx.accounts.perpetuals.get_time()?;
        vest.vest_token_account = ctx.accounts.vest_token_account.key();
        vest.vest_token_account_bump = *ctx
            .bumps
            .get("vest_token_account")
            .ok_or(ProgramError::InvalidSeeds)?;

        ctx.accounts.perpetuals.mint_tokens(
            ctx.accounts.lm_token_mint.to_account_info(),
            ctx.accounts.vest_token_account.to_account_info(),
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            vest.amount,
        )?;
    }

    // Add vest to cortex
    {
        let cortex = ctx.accounts.cortex.as_mut();

        cortex.vests.push(ctx.accounts.vest.key());
    }

    // Add governing power to Vest account and delegate to Owner
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
            "Governance - Mint {} governing token to Vest account, and delegate them to the owner: {}",
            amount,
            owner.key
        );
        add_governing_power(
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.payer.to_account_info(),
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

    Ok(0)
}
