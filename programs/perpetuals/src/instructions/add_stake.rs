//! AddStake instruction handler

use {
    crate::{
        adapters::SplGovernanceV3Adapter,
        math, program,
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
        token::authority = transfer_authority,
        seeds = [b"stake_token_account"],
        bump = cortex.stake_token_account_bump,
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

    #[account(
        mut,
        seeds = [b"governance_token_mint"],
        bump = cortex.governance_token_bump
    )]
    pub governance_token_mint: Box<Account<'info, Mint>>,

    #[account()]
    pub stake_reward_token_mint: Box<Account<'info, Mint>>,

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

    governance_program: Program<'info, SplGovernanceV3Adapter>,
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
    {
        msg!("Validate inputs");
        if params.amount == 0 {
            return Err(ProgramError::InvalidArgument.into());
        }
    }

    let did_claim = {
        let stake = ctx.accounts.stake.as_mut();
        // initialize the Stake PDA for first time stake
        if stake.stake_time == 0 {
            stake.bump = *ctx.bumps.get("stake").ok_or(ProgramError::InvalidSeeds)?;
            stake.stake_time = ctx.accounts.perpetuals.get_time()?;
            false
        } else {
            // claim reward on previously staked tokens
            // recursive program call
            let cpi_accounts = crate::cpi::accounts::ClaimStake {
                caller: ctx.accounts.owner.to_account_info(),
                owner: ctx.accounts.owner.to_account_info(),
                caller_reward_token_account: ctx
                    .accounts
                    .owner_reward_token_account
                    .to_account_info(),
                owner_reward_token_account: ctx
                    .accounts
                    .owner_reward_token_account
                    .to_account_info(),
                stake_reward_token_account: ctx
                    .accounts
                    .stake_reward_token_account
                    .to_account_info(),
                transfer_authority: ctx.accounts.transfer_authority.to_account_info(),
                stake: ctx.accounts.stake.to_account_info(),
                cortex: ctx.accounts.cortex.to_account_info(),
                perpetuals: ctx.accounts.perpetuals.to_account_info(),
                lm_token_mint: ctx.accounts.lm_token_mint.to_account_info(),
                governance_token_mint: ctx.accounts.governance_token_mint.to_account_info(),
                stake_reward_token_mint: ctx.accounts.stake_reward_token_mint.to_account_info(),
                system_program: ctx.accounts.system_program.to_account_info(),
                token_program: ctx.accounts.token_program.to_account_info(),
            };

            let cpi_program = ctx.accounts.perpetuals_program.to_account_info();
            crate::cpi::claim_stake(CpiContext::new(cpi_program, cpi_accounts))?.get()
        }
    };

    // transfer newly staked tokens to Stake PDA
    msg!("Transfer tokens");
    {
        let perpetuals = ctx.accounts.perpetuals.as_mut();
        perpetuals.transfer_tokens_from_user(
            ctx.accounts.funding_account.to_account_info(),
            ctx.accounts.stake_token_account.to_account_info(),
            ctx.accounts.owner.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            params.amount,
        )?;
    }

    // Give 1:1 (until multipliers TODO) governing power to the Stake owner
    {
        let perpetuals = ctx.accounts.perpetuals.as_mut();
        perpetuals.add_governing_power(
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.owner.to_account_info(),
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
            params.amount,
            None,
        )?;
    }

    // update Stake and Cortex data
    {
        let perpetuals = ctx.accounts.perpetuals.as_mut();
        let cortex = ctx.accounts.cortex.as_mut();
        let stake = ctx.accounts.stake.as_mut();

        if !did_claim {
            // refresh stake_time
            stake.stake_time = perpetuals.get_time()?;
        }

        // apply delta to user stake
        stake.amount = math::checked_add(stake.amount, params.amount)?;

        // apply delta to next round
        cortex.next_staking_round.total_stake =
            math::checked_add(cortex.next_staking_round.total_stake, params.amount)?;
    }

    Ok(())
}
