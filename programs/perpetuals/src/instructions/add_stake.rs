//! AddStake instruction handler

use {
    crate::{
        adapters::SplGovernanceV3Adapter,
        math, program,
        state::{
            cortex::Cortex,
            perpetuals::Perpetuals,
            staking::{LockedStake, Staking},
        },
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
        mut,
        seeds = [b"staking",
                 owner.key().as_ref()],
        bump = staking.bump
    )]
    pub staking: Box<Account<'info, Staking>>,

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

    // Amount of days to be locked for
    pub locked_days: u32,
}

pub fn add_stake(ctx: Context<AddStake>, params: &AddStakeParams) -> Result<()> {
    // validate inputs
    let staking_option = {
        msg!("Validate inputs");
        if params.amount == 0 {
            return Err(ProgramError::InvalidArgument.into());
        }

        ctx.accounts.staking.get_staking_option(params.locked_days)
    }?;

    let staking = ctx.accounts.staking.as_mut();
    let perpetuals = ctx.accounts.perpetuals.as_mut();
    let cortex = ctx.accounts.cortex.as_mut();

    let amount_with_multiplier = if staking_option.locked_days == 0 {
        //
        // Liquid staking
        //

        // @TODO, make user to not lose current round of reward when adding new tokens to liquid stake
        if staking.liquid_stake.amount > 0 {
            // Claim rewards before cdoing anything else
            {
                // claim reward on previously staked tokens
                // recursive program call
                let cpi_accounts = crate::cpi::accounts::ClaimStakes {
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
                    staking: staking.to_account_info(),
                    cortex: cortex.to_account_info(),
                    perpetuals: perpetuals.to_account_info(),
                    stake_reward_token_mint: ctx.accounts.stake_reward_token_mint.to_account_info(),
                    system_program: ctx.accounts.system_program.to_account_info(),
                    token_program: ctx.accounts.token_program.to_account_info(),
                };

                let cpi_program = ctx.accounts.perpetuals_program.to_account_info();
                crate::cpi::claim_stakes(CpiContext::new(cpi_program, cpi_accounts))?
            }

            // Drop rewards for current round, start accruing reward at next round
            cortex.current_staking_round.total_stake = math::checked_sub(
                cortex.current_staking_round.total_stake,
                staking.liquid_stake.amount_with_multiplier,
            )?;
        }

        staking.liquid_stake.amount =
            math::checked_add(staking.liquid_stake.amount, params.amount)?;

        let amount_with_multiplier = math::checked_as_u64(math::checked_div(
            math::checked_mul(
                staking.liquid_stake.amount,
                staking_option.base_reward_multiplier as u64,
            )? as u128,
            Perpetuals::BPS_POWER,
        )?)?;

        staking.liquid_stake.stake_time = perpetuals.get_time()?;
        staking.liquid_stake.base_reward_multiplier = staking_option.base_reward_multiplier;
        staking.liquid_stake.lm_token_reward_multiplier = staking_option.lm_token_reward_multiplier;
        staking.liquid_stake.vote_multiplier = staking_option.vote_multiplier;
        staking.liquid_stake.amount_with_multiplier = amount_with_multiplier;

        amount_with_multiplier
    } else {
        //
        // Locked staking
        //

        let amount_with_multiplier = math::checked_as_u64(math::checked_div(
            math::checked_mul(params.amount, staking_option.base_reward_multiplier as u64)? as u128,
            Perpetuals::BPS_POWER,
        )?)?;

        // Add the new locked staking to the list
        staking.locked_stakes.push(LockedStake {
            amount: params.amount,
            stake_time: perpetuals.get_time()?,
            claim_time: 0,

            // Transform days in seconds here
            lock_duration: math::checked_mul(staking_option.locked_days as u64, 3_600 * 24)?,
            base_reward_multiplier: staking_option.base_reward_multiplier,
            lm_token_reward_multiplier: staking_option.lm_token_reward_multiplier,
            vote_multiplier: staking_option.vote_multiplier,

            amount_with_multiplier,

            resolved: false,
        });

        amount_with_multiplier
    };

    // transfer newly staked tokens to Stake PDA
    msg!("Transfer tokens");
    {
        perpetuals.transfer_tokens_from_user(
            ctx.accounts.funding_account.to_account_info(),
            ctx.accounts.stake_token_account.to_account_info(),
            ctx.accounts.owner.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            params.amount,
        )?;
    }

    // Give governing power to the Stake owner
    {
        // Apply voting multiplier related to locking period
        let voting_power = math::checked_as_u64(math::checked_div(
            math::checked_mul(params.amount, staking_option.vote_multiplier as u64)? as u128,
            Perpetuals::BPS_POWER,
        )?)?;

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
            voting_power,
            None,
            true,
        )?;
    }

    // update Cortex data
    {
        // apply delta to next round taking into account real yield multiplier
        cortex.next_staking_round.total_stake = math::checked_add(
            cortex.next_staking_round.total_stake,
            amount_with_multiplier,
        )?;
    }

    Ok(())
}
