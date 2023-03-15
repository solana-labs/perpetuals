//! RemoveStake instruction handler

use {
    crate::{
        error::PerpetualsError,
        state::{cortex::Cortex, perpetuals::Perpetuals, stake::Stake},
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
    num::Zero,
    solana_program::program_error::ProgramError,
};

#[derive(Accounts)]
pub struct RemoveStake<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        token::mint = lm_token_mint,
        has_one = owner
    )]
    pub lm_token_account: Box<Account<'info, TokenAccount>>,

    // staking vault
    #[account(
        mut,
        token::mint = lm_token_mint,
        seeds = [b"stake_token_account"],
        bump = cortex.stake_token_account_bump
    )]
    pub stake_token_account: Box<Account<'info, TokenAccount>>,

    /// CHECK: empty PDA, authority for token accounts
    #[account(
        seeds = [b"transfer_authority"],
        bump = perpetuals.transfer_authority_bump
    )]
    pub transfer_authority: AccountInfo<'info>,

    #[account(
        mut,
        seeds = [b"stake",
                 owner.key().as_ref()],
        bump
    )]
    pub stake: Box<Account<'info, Stake>>,

    #[account(
        mut,
        seeds = [b"cortex"],
        bump = cortex.bump
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

    system_program: Program<'info, System>,
    token_program: Program<'info, Token>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Copy, Clone)]
pub struct RemoveStakeParams {
    pub amount: u64,
}

pub fn remove_stake(ctx: Context<RemoveStake>, params: &RemoveStakeParams) -> Result<()> {
    // validate inputs
    msg!("Validate inputs");
    if params.amount == 0 {
        return Err(ProgramError::InvalidArgument.into());
    }

    // verify user staked balance
    let stake = ctx.accounts.stake.as_mut();
    require!(
        stake.amount >= params.amount,
        PerpetualsError::InvalidStakeState
    );

    // claim existing rewards
    // TODO - call claim IX (let that ix verify the timestamp)

    // unstake owner's tokens
    msg!("Transfer tokens");
    let perpetuals = ctx.accounts.perpetuals.as_mut();
    perpetuals.transfer_tokens(
        ctx.accounts.stake_token_account.to_account_info(),
        ctx.accounts.lm_token_account.to_account_info(),
        ctx.accounts.transfer_authority.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        params.amount,
    )?;

    // record stake in the user `Stake` PDA and update stake time
    let stake = ctx.accounts.stake.as_mut();
    stake.amount -= params.amount;
    stake.inception_time = ctx.accounts.perpetuals.get_time()?;

    // record stake in current staking round
    // (no double count as the previous amount is counted as claimed after the claim IX call, even if called during the same round)
    let cortex = ctx.accounts.cortex.as_mut();
    cortex.get_latest_staking_round_mut()?.total_stake += stake.amount;

    // cleanup the stake PDA if all stake has been removed
    if stake.amount.is_zero() {
        stake.amount = 0;
        stake.bump = 0;
        stake.inception_time = 0;
        stake.close(ctx.accounts.owner.to_account_info())?;
    }

    Ok(())
}
