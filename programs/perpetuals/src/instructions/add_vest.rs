//! AddVest instruction handler

use {
    crate::state::{
        cortex::Cortex,
        multisig::{AdminInstruction, Multisig},
        perpetuals::Perpetuals,
        vest::Vest,
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token},
};

#[derive(Accounts)]
#[instruction(params: AddVestParams)]
pub struct AddVest<'info> {
    #[account(mut)]
    pub admin: Signer<'info>,

    #[account()]
    pub owner: AccountInfo<'info>,

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
        realloc = Cortex::LEN + (cortex.vests.len() + 1) * std::mem::size_of::<Vest>(),
        realloc::payer = admin,
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
        bump
    )]
    pub lm_token_mint: Box<Account<'info, Mint>>,

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

    // record vest data
    let cortex = ctx.accounts.cortex.as_mut();
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

    // mint token (TODO: change the dest account -- TEST still failing without this)
    // let lm_amount = vest.amount;
    // msg!("LM tokens to mint: {}", lm_amount);

    // // mint lp tokens
    // ctx.accounts.perpetuals.mint_tokens(
    //     ctx.accounts.lm_token_mint.to_account_info(),
    //     ctx.accounts.<Put the governance acc>.to_account_info(),
    //     ctx.accounts.transfer_authority.to_account_info(),
    //     ctx.accounts.token_program.to_account_info(),
    //     lm_amount,
    // )?;

    // TODO
    // 2) transfer tokens to delegate Governance accounts for the beneficiary

    cortex.vests.push(ctx.accounts.vest.key());

    Ok(0)
}
