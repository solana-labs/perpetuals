//! RemoveStake instruction handler

use {
    crate::{
        error::PerpetualsError,
        math, program,
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
        mut,
        seeds = [b"stake",
                 owner.key().as_ref()],
        bump = stake.bump
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
pub struct RemoveStakeParams {
    pub amount: u64,
}

pub fn remove_stake(ctx: Context<RemoveStake>, params: &RemoveStakeParams) -> Result<()> {
    // validate inputs
    {
        msg!("Validate inputs");
        if params.amount == 0 {
            return Err(ProgramError::InvalidArgument.into());
        }
    }

    // verify user staked balance
    {
        let stake = ctx.accounts.stake.as_mut();
        require!(
            stake.amount >= params.amount,
            PerpetualsError::InvalidStakeState
        );
    }

    // claim existing rewards
    let did_claim = {
        // calling the program itself through CPI to enforce parity with cpi API
        let cpi_accounts = crate::cpi::accounts::ClaimStake {
            caller: ctx.accounts.owner.to_account_info(),
            owner: ctx.accounts.owner.to_account_info(),
            caller_reward_token_account: ctx.accounts.owner_reward_token_account.to_account_info(),
            owner_reward_token_account: ctx.accounts.owner_reward_token_account.to_account_info(),
            stake_token_account: ctx.accounts.stake_token_account.to_account_info(),
            stake_reward_token_account: ctx.accounts.stake_reward_token_account.to_account_info(),
            transfer_authority: ctx.accounts.transfer_authority.to_account_info(),
            stake: ctx.accounts.stake.to_account_info(),
            cortex: ctx.accounts.cortex.to_account_info(),
            perpetuals: ctx.accounts.perpetuals.to_account_info(),
            lm_token_mint: ctx.accounts.lm_token_mint.to_account_info(),
            stake_reward_token_mint: ctx.accounts.stake_reward_token_mint.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };

        let cpi_program = ctx.accounts.perpetuals_program.to_account_info();
        crate::cpi::claim_stake(CpiContext::new(cpi_program, cpi_accounts))?.get()
    };

    // unstake owner's tokens
    {
        msg!("Transfer tokens");
        let perpetuals = ctx.accounts.perpetuals.as_mut();
        perpetuals.transfer_tokens(
            ctx.accounts.stake_token_account.to_account_info(),
            ctx.accounts.lm_token_account.to_account_info(),
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            params.amount,
        )?;
    }

    // update Stake and Cortex data
    {
        let perpetuals = ctx.accounts.perpetuals.as_mut();
        let cortex = ctx.accounts.cortex.as_mut();
        let stake = ctx.accounts.stake.as_mut();

        if !did_claim {
            // forfeit current round participation, if any
            if stake.qualifies_for_rewards_from(&cortex.current_staking_round) {
                // remove previous stake from current staking round
                cortex.current_staking_round.total_stake =
                    math::checked_sub(cortex.current_staking_round.total_stake, stake.amount)?;
            }

            // refresh stake_time
            stake.stake_time = perpetuals.get_time()?;
        }

        // apply delta to user stake
        stake.amount = math::checked_sub(stake.amount, params.amount)?;

        // apply delta to next round
        cortex.next_staking_round.total_stake =
            math::checked_sub(cortex.next_staking_round.total_stake, params.amount)?;

        msg!(
            "Cortex.resolved_staking_rounds after remove stake {:?}",
            cortex.resolved_staking_rounds
        );
        msg!(
            "Cortex.current_staking_round after remove stake {:?}",
            cortex.current_staking_round
        );
        msg!(
            "Cortex.next_staking_round after remove stake {:?}",
            cortex.next_staking_round
        );
        msg!("STATE after remove stake {:?}", stake);
    }

    // cleanup the stake PDA if stake.amount is zero
    {
        let stake = ctx.accounts.stake.as_mut();
        if stake.amount.is_zero() {
            stake.amount = 0;
            stake.bump = 0;
            stake.stake_time = 0;
            stake.close(ctx.accounts.owner.to_account_info())?;
        }
    }

    Ok(())
}
