//! ResolveLockedStakes instruction handler

use {
    crate::{
        adapters::SplGovernanceV3Adapter,
        math, program,
        state::{cortex::Cortex, perpetuals::Perpetuals, staking::Staking},
    },
    anchor_lang::prelude::*,
    anchor_spl::token::{Mint, Token},
};

#[derive(Accounts)]
pub struct ResolveLockedStakes<'info> {
    // TODO:
    // Caller should be the program iself
    #[account(mut)]
    pub caller: Signer<'info>,

    /// CHECK: verified through the `stake` account seed derivation
    #[account(mut)]
    pub owner: AccountInfo<'info>,

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

// Resolving a stake means cancelling the governing power related to the stake and stopping to accrue rewards
// A stake can be resolved when its locking period have ended
// After a stake is resolved, it can be removed by the user to retrieve its tokens
pub fn resolve_locked_stakes(ctx: Context<ResolveLockedStakes>) -> Result<()> {
    let staking = ctx.accounts.staking.as_mut();
    let perpetuals = ctx.accounts.perpetuals.as_mut();
    let cortex = ctx.accounts.cortex.as_mut();

    let current_time = perpetuals.get_time()?;

    for mut locked_stake in staking.locked_stakes.iter_mut() {
        if locked_stake.has_ended(current_time) && !locked_stake.resolved {
            // Revoke governing power allocated to the stake
            {
                let voting_power = math::checked_as_u64(math::checked_div(
                    math::checked_mul(locked_stake.amount, locked_stake.vote_multiplier as u64)?
                        as u128,
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

            // forfeit current round participation
            cortex.current_staking_round.total_stake = math::checked_sub(
                cortex.current_staking_round.total_stake,
                locked_stake.amount_with_multiplier,
            )?;

            msg!(
                "cortex.next_staking_round.total_stake: {}",
                cortex.next_staking_round.total_stake
            );

            // remove staked tokens from next round
            cortex.next_staking_round.total_stake = math::checked_sub(
                cortex.next_staking_round.total_stake,
                locked_stake.amount_with_multiplier,
            )?;

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

            msg!(
                ">>>> locked_stake: lock_duration: {}, stake_time: {} ",
                locked_stake.lock_duration,
                locked_stake.stake_time
            );

            locked_stake.resolved = true;
        }
    }

    Ok(())
}
