//! AddStake instruction handler

use {
    crate::{
        instructions, math, program,
        state::{cortex::Cortex, perpetuals::Perpetuals, stake::Stake},
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
    solana_program::program_error::ProgramError,
};

#[derive(Accounts)]
pub struct AddStake<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        token::mint = lm_token_mint,
        has_one = owner
    )]
    pub funding_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = stake_reward_token_mint,
        has_one = owner
    )]
    pub owner_reward_token_account: Box<Account<'info, TokenAccount>>,

    // staked token vault
    #[account(
        mut,
        token::mint = lm_token_mint,
        seeds = [b"stake_token_account"],
        bump = cortex.stake_token_account_bump
    )]
    pub stake_token_account: Box<Account<'info, TokenAccount>>,

    // staking reward token vault
    #[account(
        mut,
        token::mint = stake_reward_token_mint,
        seeds = [b"stake_reward_token_account"],
        bump = cortex.stake_reward_token_account_bump
    )]
    pub stake_reward_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    #[account(
        init_if_needed,
        payer = owner,
        space = Stake::LEN,
        seeds = [b"stake",
                 owner.key().as_ref()],
        bump
    )]
    pub stake: Box<Account<'info, Stake>>,

    #[account(
        mut,
        seeds = [b"cortex"],
        bump = cortex.bump,
        has_one = stake_reward_token_mint
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

    #[account()]
    pub stake_reward_token_mint: Box<Account<'info, Mint>>,

    perpetuals_program: Program<'info, program::Perpetuals>,
    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone)]
pub struct AddStakeParams {
    pub amount: u64,
}

pub fn add_stake(ctx: Context<AddStake>, params: &AddStakeParams) -> Result<()> {
    // validate inputs
    msg!("Validate inputs");
    if params.amount == 0 {
        return Err(ProgramError::InvalidArgument.into());
    }

    // initialize Stake PDA if needed, or claim existing rewards
    let stake = ctx.accounts.stake.as_mut();
    if stake.stake_time == 0 {
        stake.bump = *ctx.bumps.get("stake").ok_or(ProgramError::InvalidSeeds)?;
        stake.stake_time = ctx.accounts.perpetuals.get_time()?;
    } else {
        let cpi_accounts = instructions::claim_stake::ClaimStake {
            caller: ctx.accounts.owner.clone(),
            owner: ctx.accounts.owner.to_account_info(),
            caller_reward_token_account: ctx.accounts.owner_reward_token_account.clone(),
            owner_reward_token_account: ctx.accounts.owner_reward_token_account.clone(),
            stake_token_account: ctx.accounts.stake_token_account.clone(),
            stake_reward_token_account: ctx.accounts.stake_reward_token_account.clone(),
            transfer_authority: ctx.accounts.transfer_authority.clone(),
            stake: ctx.accounts.stake.clone(),
            cortex: ctx.accounts.cortex.clone(),
            perpetuals: ctx.accounts.perpetuals.clone(),
            lm_token_mint: ctx.accounts.lm_token_mint.clone(),
            stake_reward_token_mint: ctx.accounts.stake_reward_token_mint.clone(),
            system_program: ctx.accounts.system_program.clone(),
            token_program: ctx.accounts.token_program.clone(),
        };

        let cpi_program = ctx.accounts.perpetuals_program.to_account_info();
        instructions::claim_stake(CpiContext::new(cpi_program, cpi_accounts).into())?;
    }

    // stake owner's tokens
    msg!("Transfer tokens");
    let perpetuals = ctx.accounts.perpetuals.as_mut();
    perpetuals.transfer_tokens_from_user(
        ctx.accounts.funding_account.to_account_info(),
        ctx.accounts.stake_token_account.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        params.amount,
    )?;

    // update Stake and Cortex data
    // note: the update to the current round has already been done in the `claim` ix call,
    //       only the delta is processed here
    let cortex = ctx.accounts.cortex.as_mut();
    let stake = ctx.accounts.stake.as_mut();

    // record updated stake amount in the user `Stake` PDA
    stake.amount = math::checked_add(stake.amount, params.amount)?;

    // add new stake to next round
    cortex.next_staking_round.total_stake =
        math::checked_add(cortex.next_staking_round.total_stake, params.amount)?;

    Ok(())
}

// impl<'info> AddStake<'info> {
//     pub fn into_claim_stake_context(
//         &self,
//     ) -> CpiContext<'_, '_, '_, 'info, perpetuals::cpi::accounts::ClaimStake<'info>> {
//     }
// }
