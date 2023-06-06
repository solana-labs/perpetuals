//! RemoveStake instruction handler

use {
    crate::{
        adapters::SplGovernanceV3Adapter,
        error::PerpetualsError,
        math, program,
        state::{cortex::Cortex, perpetuals::Perpetuals, staking::Staking},
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token, TokenAccount},
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
        token::authority = transfer_authority,
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
pub struct RemoveStakeParams {
    // Liquid staking
    pub remove_liquid_stake: bool,
    pub amount: Option<u64>,

    // Locked staking
    pub remove_locked_stake: bool,
    pub locked_stake_index: Option<usize>,
}

// Remove one stake at a time
pub fn remove_stake(ctx: Context<RemoveStake>, params: &RemoveStakeParams) -> Result<()> {
    // validate inputs
    {
        msg!("Validate inputs");

        // Only one staking to end at a time
        if (params.remove_liquid_stake && params.remove_locked_stake)
            || (!params.remove_liquid_stake && !params.remove_locked_stake)
        {
            return Err(ProgramError::InvalidArgument.into());
        }

        // missing index when removing locked stake
        if params.remove_locked_stake && params.locked_stake_index.is_none() {
            return Err(ProgramError::InvalidArgument.into());
        }

        // missing amount when removing liquid stake
        if params.remove_liquid_stake && (params.amount.is_none() || params.amount.unwrap() == 0) {
            return Err(ProgramError::InvalidArgument.into());
        }
    }

    // claim existing rewards before removing the stake
    {
        // calling the program itself through CPI to enforce parity with cpi API
        let cpi_accounts = crate::cpi::accounts::ClaimStakes {
            caller: ctx.accounts.owner.to_account_info(),
            owner: ctx.accounts.owner.to_account_info(),
            caller_reward_token_account: ctx.accounts.owner_reward_token_account.to_account_info(),
            owner_reward_token_account: ctx.accounts.owner_reward_token_account.to_account_info(),
            stake_reward_token_account: ctx.accounts.stake_reward_token_account.to_account_info(),
            transfer_authority: ctx.accounts.transfer_authority.to_account_info(),
            staking: ctx.accounts.staking.to_account_info(),
            cortex: ctx.accounts.cortex.to_account_info(),
            perpetuals: ctx.accounts.perpetuals.to_account_info(),
            stake_reward_token_mint: ctx.accounts.stake_reward_token_mint.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        };

        let cpi_program = ctx.accounts.perpetuals_program.to_account_info();
        crate::cpi::claim_stakes(CpiContext::new(cpi_program, cpi_accounts))?
    }

    let staking = ctx.accounts.staking.as_mut();
    let perpetuals = ctx.accounts.perpetuals.as_mut();
    let cortex = ctx.accounts.cortex.as_mut();

    let token_amount_to_unstake = if params.remove_liquid_stake {
        //
        // Handle liquid staking
        //

        let token_amount_to_unstake = params.amount.unwrap();

        // verify user staked balance
        {
            require!(
                staking.liquid_stake.amount >= token_amount_to_unstake,
                PerpetualsError::InvalidStakeState
            );
        }

        // Revoke governing power allocated to the stake
        {
            let voting_power = math::checked_as_u64(math::checked_div(
                math::checked_mul(
                    token_amount_to_unstake,
                    staking.liquid_stake.vote_multiplier as u64,
                )? as u128,
                Perpetuals::BPS_POWER,
            )?)?;

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
                voting_power,
            )?;
        }

        // apply delta to user stake
        staking.liquid_stake.amount =
            math::checked_sub(staking.liquid_stake.amount, token_amount_to_unstake)?;

        // Apply delta to current and next round
        {
            let real_yield_unstake_amount = math::checked_as_u64(math::checked_div(
                math::checked_mul(
                    token_amount_to_unstake,
                    staking.liquid_stake.base_reward_multiplier as u64,
                )? as u128,
                Perpetuals::BPS_POWER,
            )?)?;

            // forfeit current round participation, if any
            if staking
                .liquid_stake
                .qualifies_for_rewards_from(&cortex.current_staking_round)
            {
                cortex.current_staking_round.total_stake = math::checked_sub(
                    cortex.current_staking_round.total_stake,
                    real_yield_unstake_amount,
                )?;
            }

            // apply delta to next round
            cortex.next_staking_round.total_stake = math::checked_sub(
                cortex.next_staking_round.total_stake,
                real_yield_unstake_amount,
            )?;
        }

        token_amount_to_unstake
    } else {
        //
        // Handle locked staking
        //

        let locked_stake = staking
            .locked_stakes
            .get(params.locked_stake_index.unwrap())
            .ok_or(PerpetualsError::CannotFoundStake)?;

        // Check the stake have ended and have been resolved
        {
            let current_time = ctx.accounts.perpetuals.get_time()?;
            require!(
                locked_stake.has_ended(current_time) && locked_stake.resolved,
                PerpetualsError::UnresolvedStake
            );
        }

        let token_amount_to_unstake = locked_stake.amount;

        // Remove the stake from the list
        staking
            .locked_stakes
            .remove(params.locked_stake_index.unwrap());

        token_amount_to_unstake
    };

    // Unstake owner's tokens
    {
        msg!("Transfer tokens");
        let perpetuals = ctx.accounts.perpetuals.as_mut();

        perpetuals.transfer_tokens(
            ctx.accounts.stake_token_account.to_account_info(),
            ctx.accounts.lm_token_account.to_account_info(),
            ctx.accounts.transfer_authority.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            token_amount_to_unstake,
        )?;
    }

    /*
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
            stake_reward_token_account: ctx.accounts.stake_reward_token_account.to_account_info(),
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

    // Revoke 1:1 (until multipliers TODO) governing power to the Stake owner
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
    }*/

    Ok(())
}
