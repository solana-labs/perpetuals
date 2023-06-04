//! AddVest instruction handler

use {
    crate::{
        adapters::{self, CreateTokenOwnerRecord, SplGovernanceV3Adapter},
        error::PerpetualsError,
        state::{
            cortex::Cortex,
            multisig::{AdminInstruction, Multisig},
            perpetuals::Perpetuals,
            vest::Vest,
        },
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token},
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

const SEVEN_DAYS_IN_SECONDS: i64 = 3_600 * 24 * 7;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AddVestParams {
    pub amount: u64,
    pub unlock_start_timestamp: i64,
    pub unlock_end_timestamp: i64,
}

pub fn add_vest<'info>(
    ctx: Context<'_, '_, '_, 'info, AddVest<'info>>,
    params: &AddVestParams,
) -> Result<u8> {
    // validate inputs
    {
        if params.amount == 0 || params.unlock_end_timestamp <= params.unlock_start_timestamp {
            return Err(ProgramError::InvalidArgument.into());
        }

        let current_time = ctx.accounts.perpetuals.get_time()?;

        // Unlock must end in minimum 7 days
        require!(
            params.unlock_end_timestamp >= (current_time + SEVEN_DAYS_IN_SECONDS),
            PerpetualsError::InvalidVestingUnlockTime
        );

        // Vesting must be at least 7 days long
        require!(
            (params.unlock_end_timestamp - params.unlock_start_timestamp) >= SEVEN_DAYS_IN_SECONDS,
            PerpetualsError::InvalidVestingUnlockTime
        );
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

        // return error if vest is already initialized
        if vest.amount != 0 && vest.claimed_amount < vest.amount {
            return Err(ProgramError::AccountAlreadyInitialized.into());
        }

        msg!(
            "Record vest: amount {}, owner {}, unlock_start_timestamp {}, unlock_end_timestamp: {}",
            params.amount,
            ctx.accounts.owner.key,
            params.unlock_start_timestamp,
            params.unlock_end_timestamp,
        );

        vest.amount = params.amount;
        vest.unlock_start_timestamp = params.unlock_start_timestamp;
        vest.unlock_end_timestamp = params.unlock_end_timestamp;
        vest.claimed_amount = 0;
        vest.last_claim_timestamp = 0;
        vest.owner = ctx.accounts.owner.key();
        vest.bump = *ctx.bumps.get("vest").ok_or(ProgramError::InvalidSeeds)?;
    }

    // Add vest to cortex
    {
        let cortex = ctx.accounts.cortex.as_mut();

        cortex.vests.push(ctx.accounts.vest.key());
    }

    // Give 1:1 governing power to the Vest owner (signed by the mint)
    {
        let perpetuals = ctx.accounts.perpetuals.as_mut();
        let mint_seeds: &[&[u8]] = &[
            b"governance_token_mint",
            &[ctx.accounts.cortex.governance_token_bump],
        ];

        // due to some limitation in the governance code (a check that prevent depositing
        // governance power when the owner is not signing the TX), we have to call
        // create_token_owner_record first to bypass the signer check limitation on
        // the token owner not signing this TX necesarily
        {
            let cpi_accounts = CreateTokenOwnerRecord {
                realm: ctx.accounts.governance_realm.to_account_info(),
                governing_token_owner: ctx.accounts.owner.to_account_info(),
                governing_token_owner_record: ctx
                    .accounts
                    .governance_governing_token_owner_record
                    .to_account_info(),
                governing_token_mint: ctx.accounts.governance_token_mint.to_account_info(),
                payer: ctx.accounts.payer.to_account_info(),
            };

            let cpi_program = ctx.accounts.governance_program.to_account_info();
            adapters::create_token_owner_record(CpiContext::new(cpi_program, cpi_accounts))?;
        }

        perpetuals.add_governing_power(
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.payer.to_account_info(),
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
            ctx.accounts.vest.amount,
            Some(mint_seeds),
            false,
        )?;
    }

    Ok(0)
}
