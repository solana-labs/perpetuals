use {
    super::{
        cortex::{Cortex, FeeDistribution},
        custody::Custody,
        pool::Pool,
        staking::Staking,
    },
    crate::{adapters, instructions::SwapParams},
    anchor_lang::prelude::*,
    anchor_spl::token::{Burn, Mint, MintTo, TokenAccount, Transfer},
    num_traits::Zero,
    solana_program::account_info::AccountInfo,
    spl_governance::state::token_owner_record::get_token_owner_record_data,
    std::cmp::min,
};

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct PriceAndFee {
    pub price: u64,
    pub fee: u64,
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct AmountAndFee {
    pub amount: u64,
    pub fee: u64,
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct NewPositionPricesAndFee {
    pub entry_price: u64,
    pub liquidation_price: u64,
    pub fee: u64,
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct SwapAmountAndFees {
    pub amount_out: u64,
    pub fee_in: u64,
    pub fee_out: u64,
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct ProfitAndLoss {
    pub profit: u64,
    pub loss: u64,
}

#[derive(Copy, Clone, PartialEq, AnchorSerialize, AnchorDeserialize, Default, Debug)]
pub struct Permissions {
    pub allow_swap: bool,
    pub allow_add_liquidity: bool,
    pub allow_remove_liquidity: bool,
    pub allow_open_position: bool,
    pub allow_close_position: bool,
    pub allow_pnl_withdrawal: bool,
    pub allow_collateral_withdrawal: bool,
    pub allow_size_change: bool,
}

#[account]
#[derive(Default, Debug)]
pub struct Perpetuals {
    pub permissions: Permissions,
    pub pools: Vec<Pubkey>,

    pub transfer_authority_bump: u8,
    pub perpetuals_bump: u8,
    // time of inception, also used as current wall clock time for testing
    pub inception_time: i64,
}

impl anchor_lang::Id for Perpetuals {
    fn id() -> Pubkey {
        crate::ID
    }
}

impl Perpetuals {
    pub const LEN: usize = 8 + std::mem::size_of::<Perpetuals>();
    pub const BPS_DECIMALS: u8 = 4;
    pub const BPS_POWER: u128 = 10u64.pow(Self::BPS_DECIMALS as u32) as u128;
    pub const PRICE_DECIMALS: u8 = 6;
    pub const USD_DECIMALS: u8 = 6;
    pub const LP_DECIMALS: u8 = Self::USD_DECIMALS;
    pub const RATE_DECIMALS: u8 = 9;
    pub const RATE_POWER: u128 = 10u64.pow(Self::RATE_DECIMALS as u32) as u128;

    pub fn validate(&self) -> bool {
        true
    }

    pub fn get_time(&self) -> Result<i64> {
        let time = solana_program::sysvar::clock::Clock::get()?.unix_timestamp;
        if time > 0 {
            Ok(time)
        } else {
            Err(ProgramError::InvalidAccountData.into())
        }
    }

    pub fn transfer_tokens<'info>(
        &self,
        from: AccountInfo<'info>,
        to: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        token_program: AccountInfo<'info>,
        amount: u64,
    ) -> Result<()> {
        let authority_seeds: &[&[&[u8]]] =
            &[&[b"transfer_authority", &[self.transfer_authority_bump]]];

        msg!(
            "Transfer {} tokens from {} to {}",
            amount,
            from.key(),
            to.key()
        );

        let context = CpiContext::new(
            token_program,
            Transfer {
                from,
                to,
                authority,
            },
        )
        .with_signer(authority_seeds);

        anchor_spl::token::transfer(context, amount)
    }

    pub fn transfer_tokens_from_user<'info>(
        &self,
        from: AccountInfo<'info>,
        to: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        token_program: AccountInfo<'info>,
        amount: u64,
    ) -> Result<()> {
        let context = CpiContext::new(
            token_program,
            Transfer {
                from,
                to,
                authority,
            },
        );
        anchor_spl::token::transfer(context, amount)
    }

    pub fn mint_tokens<'info>(
        &self,
        mint: AccountInfo<'info>,
        to: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        token_program: AccountInfo<'info>,
        amount: u64,
    ) -> Result<()> {
        let authority_seeds: &[&[&[u8]]] =
            &[&[b"transfer_authority", &[self.transfer_authority_bump]]];

        let context = CpiContext::new(
            token_program,
            MintTo {
                mint,
                to,
                authority,
            },
        )
        .with_signer(authority_seeds);

        anchor_spl::token::mint_to(context, amount)
    }

    pub fn burn_tokens<'info>(
        &self,
        mint: AccountInfo<'info>,
        from: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        token_program: AccountInfo<'info>,
        amount: u64,
    ) -> Result<()> {
        let context = CpiContext::new(
            token_program,
            Burn {
                mint,
                from,
                authority,
            },
        );

        anchor_spl::token::burn(context, amount)
    }

    pub fn is_empty_account(account_info: &AccountInfo) -> Result<bool> {
        Ok(account_info.try_data_is_empty()? || account_info.try_lamports()? == 0)
    }

    pub fn close_token_account<'info>(
        receiver: AccountInfo<'info>,
        token_account: AccountInfo<'info>,
        token_program: AccountInfo<'info>,
        authority: AccountInfo<'info>,
        seeds: &[&[&[u8]]],
    ) -> Result<()> {
        let cpi_accounts = anchor_spl::token::CloseAccount {
            account: token_account,
            destination: receiver,
            authority,
        };
        let cpi_context = anchor_lang::context::CpiContext::new(token_program, cpi_accounts);

        anchor_spl::token::close_account(cpi_context.with_signer(seeds))
    }

    pub fn transfer_sol_from_owned<'a>(
        program_owned_source_account: AccountInfo<'a>,
        destination_account: AccountInfo<'a>,
        amount: u64,
    ) -> Result<()> {
        **destination_account.try_borrow_mut_lamports()? = destination_account
            .try_lamports()?
            .checked_add(amount)
            .ok_or(ProgramError::InsufficientFunds)?;

        let source_balance = program_owned_source_account.try_lamports()?;
        **program_owned_source_account.try_borrow_mut_lamports()? = source_balance
            .checked_sub(amount)
            .ok_or(ProgramError::InsufficientFunds)?;

        Ok(())
    }

    pub fn transfer_sol<'a>(
        source_account: AccountInfo<'a>,
        destination_account: AccountInfo<'a>,
        system_program: AccountInfo<'a>,
        amount: u64,
    ) -> Result<()> {
        let cpi_accounts = anchor_lang::system_program::Transfer {
            from: source_account,
            to: destination_account,
        };
        let cpi_context = anchor_lang::context::CpiContext::new(system_program, cpi_accounts);

        anchor_lang::system_program::transfer(cpi_context, amount)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn realloc<'a>(
        funding_account: AccountInfo<'a>,
        target_account: AccountInfo<'a>,
        system_program: AccountInfo<'a>,
        new_len: usize,
        zero_init: bool,
    ) -> Result<()> {
        let new_minimum_balance = Rent::get()?.minimum_balance(new_len);
        let lamports_diff = new_minimum_balance.saturating_sub(target_account.try_lamports()?);

        Perpetuals::transfer_sol(
            funding_account,
            target_account.clone(),
            system_program,
            lamports_diff,
        )?;

        target_account
            .realloc(new_len, zero_init)
            .map_err(|_| ProgramError::InvalidRealloc.into())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn distribute_fees<'a>(
        &self,
        swap_required: bool,
        fee_distribution: FeeDistribution,
        transfer_authority: AccountInfo<'a>,
        custody_token_account: &mut Account<'a, TokenAccount>,
        lm_token_account: &mut Account<'a, TokenAccount>,
        cortex: &mut Account<'a, Cortex>,
        perpetuals: &mut Account<'a, Perpetuals>,
        pool: &mut Account<'a, Pool>,
        custody: &mut Account<'a, Custody>,
        custody_oracle_account: AccountInfo<'a>,
        staking_reward_token_custody: &mut Account<'a, Custody>,
        staking_reward_token_custody_oracle_account: AccountInfo<'a>,
        staking_reward_token_custody_token_account: &mut Account<'a, TokenAccount>,
        lm_staking_reward_token_vault: &mut Account<'a, TokenAccount>,
        lp_staking_reward_token_vault: &mut Account<'a, TokenAccount>,
        staking_reward_token_mint: &mut Account<'a, Mint>,
        lm_staking: &mut Account<'a, Staking>,
        lp_staking: &mut Account<'a, Staking>,
        lm_token_mint: &mut Account<'a, Mint>,
        lp_token_mint: &mut Account<'a, Mint>,
        token_program: AccountInfo<'a>,
        perpetuals_program: AccountInfo<'a>,
    ) -> Result<()> {
        {
            if !fee_distribution.lm_stakers_fee.is_zero() {
                // It is possible that the custody targeted by the function and the stake_reward one are the same, in that
                // case we need to only use one else there are some complication when saving state at the end.
                //
                // if the collected fees are in the right denomination, skip swap
                if !swap_required {
                    msg!("Transfer collected fees to stake vault (no swap)");
                    self.transfer_tokens(
                        custody_token_account.to_account_info(),
                        lm_staking_reward_token_vault.to_account_info(),
                        transfer_authority.clone(),
                        token_program.clone(),
                        fee_distribution.lm_stakers_fee,
                    )?;
                } else {
                    // swap the collected fee_amount to stable and send to staking rewards
                    msg!("Swap collected fees to stake reward mint internally");
                    self.internal_swap(
                        transfer_authority.clone(),
                        custody_token_account.to_account_info(),
                        lm_staking_reward_token_vault.to_account_info(),
                        lm_token_account.to_account_info(),
                        cortex.to_account_info(),
                        perpetuals.to_account_info(),
                        pool.to_account_info(),
                        custody.to_account_info(),
                        custody_oracle_account.clone(),
                        custody_token_account.to_account_info(),
                        staking_reward_token_custody.to_account_info(),
                        staking_reward_token_custody_oracle_account.clone(),
                        staking_reward_token_custody_token_account.to_account_info(),
                        staking_reward_token_custody.to_account_info(),
                        staking_reward_token_custody_oracle_account.clone(),
                        staking_reward_token_custody_token_account.to_account_info(),
                        lm_staking_reward_token_vault.to_account_info(),
                        lp_staking_reward_token_vault.to_account_info(),
                        staking_reward_token_mint.to_account_info(),
                        lm_staking.to_account_info(),
                        lp_staking.to_account_info(),
                        lm_token_mint.to_account_info(),
                        lp_token_mint.to_account_info(),
                        token_program.clone(),
                        perpetuals_program.clone(),
                        SwapParams {
                            amount_in: fee_distribution.lm_stakers_fee,
                            min_amount_out: 0,
                        },
                    )?;
                }
            }
        }

        // Reload all used accounts
        {
            custody_token_account.reload()?;
            lm_token_account.reload()?;
            cortex.reload()?;
            perpetuals.reload()?;
            pool.reload()?;
            custody.reload()?;
            staking_reward_token_custody.reload()?;
            staking_reward_token_custody_token_account.reload()?;
            lm_staking_reward_token_vault.reload()?;
            lp_staking_reward_token_vault.reload()?;
            staking_reward_token_mint.reload()?;
            lm_staking.reload()?;
            lp_staking.reload()?;
            lm_token_mint.reload()?;
            lp_token_mint.reload()?;
        }

        // redistribute to ALP locked stakers
        {
            if !fee_distribution.locked_lp_stakers_fee.is_zero() {
                // It is possible that the custody targeted by the function and the stake_reward one are the same, in that
                // case we need to only use one else there are some complication when saving state at the end.
                //
                // if the collected fees are in the right denomination, skip swap
                if !swap_required {
                    msg!("Transfer collected fees to stake vault (no swap)");
                    self.transfer_tokens(
                        custody_token_account.to_account_info(),
                        lp_staking_reward_token_vault.to_account_info(),
                        transfer_authority.clone(),
                        token_program.clone(),
                        fee_distribution.locked_lp_stakers_fee,
                    )?;
                } else {
                    // swap the collected fee_amount to stable and send to staking rewards
                    msg!("Swap collected fees to stake reward mint internally");
                    self.internal_swap(
                        transfer_authority.clone(),
                        custody_token_account.to_account_info(),
                        lp_staking_reward_token_vault.to_account_info(),
                        lm_token_account.to_account_info(),
                        cortex.to_account_info(),
                        perpetuals.to_account_info(),
                        pool.to_account_info(),
                        custody.to_account_info(),
                        custody_oracle_account.clone(),
                        custody_token_account.to_account_info(),
                        staking_reward_token_custody.to_account_info(),
                        staking_reward_token_custody_oracle_account.clone(),
                        staking_reward_token_custody_token_account.to_account_info(),
                        staking_reward_token_custody.to_account_info(),
                        staking_reward_token_custody_oracle_account.clone(),
                        staking_reward_token_custody_token_account.to_account_info(),
                        lm_staking_reward_token_vault.to_account_info(),
                        lp_staking_reward_token_vault.to_account_info(),
                        staking_reward_token_mint.to_account_info(),
                        lm_staking.to_account_info(),
                        lp_staking.to_account_info(),
                        lm_token_mint.to_account_info(),
                        lp_token_mint.to_account_info(),
                        token_program.clone(),
                        perpetuals_program.clone(),
                        SwapParams {
                            amount_in: fee_distribution.locked_lp_stakers_fee,
                            min_amount_out: 0,
                        },
                    )?;
                }
            }
        }

        // Reload all used accounts
        {
            custody_token_account.reload()?;
            lm_token_account.reload()?;
            cortex.reload()?;
            perpetuals.reload()?;
            pool.reload()?;
            custody.reload()?;
            staking_reward_token_custody.reload()?;
            staking_reward_token_custody_token_account.reload()?;
            lm_staking_reward_token_vault.reload()?;
            lp_staking_reward_token_vault.reload()?;
            staking_reward_token_mint.reload()?;
            lm_staking.reload()?;
            lp_staking.reload()?;
            lm_token_mint.reload()?;
            lp_token_mint.reload()?;
        }

        Ok(())
    }

    // recursive swap CPI
    #[allow(clippy::too_many_arguments)]
    pub fn internal_swap<'a>(
        &self,
        authority: AccountInfo<'a>,
        funding_account: AccountInfo<'a>,
        receiving_account: AccountInfo<'a>,
        lm_token_account: AccountInfo<'a>,
        cortex: AccountInfo<'a>,
        perpetuals: AccountInfo<'a>,
        pool: AccountInfo<'a>,
        receiving_custody: AccountInfo<'a>,
        receiving_custody_oracle_account: AccountInfo<'a>,
        receiving_custody_token_account: AccountInfo<'a>,
        dispensing_custody: AccountInfo<'a>,
        dispensing_custody_oracle_account: AccountInfo<'a>,
        dispensing_custody_token_account: AccountInfo<'a>,
        staking_reward_token_custody: AccountInfo<'a>,
        staking_reward_token_custody_oracle_account: AccountInfo<'a>,
        staking_reward_token_custody_token_account: AccountInfo<'a>,
        lm_staking_reward_token_vault: AccountInfo<'a>,
        lp_staking_reward_token_vault: AccountInfo<'a>,
        staking_reward_token_mint: AccountInfo<'a>,
        lm_staking: AccountInfo<'a>,
        lp_staking: AccountInfo<'a>,
        lm_token_mint: AccountInfo<'a>,
        lp_token_mint: AccountInfo<'a>,
        token_program: AccountInfo<'a>,
        perpetuals_program: AccountInfo<'a>,
        params: SwapParams,
    ) -> Result<()> {
        let authority_seeds: &[&[&[u8]]] =
            &[&[b"transfer_authority", &[self.transfer_authority_bump]]];

        let cpi_accounts = crate::cpi::accounts::Swap {
            owner: authority.clone(),
            funding_account,
            receiving_account,
            lm_token_account,
            transfer_authority: authority,
            cortex,
            perpetuals,
            lm_staking,
            lp_staking,
            pool,
            staking_reward_token_custody,
            staking_reward_token_custody_oracle_account,
            staking_reward_token_custody_token_account,
            receiving_custody,
            receiving_custody_oracle_account,
            receiving_custody_token_account,
            dispensing_custody,
            dispensing_custody_oracle_account,
            dispensing_custody_token_account,
            lm_staking_reward_token_vault,
            lp_staking_reward_token_vault,
            lm_token_mint,
            lp_token_mint,
            staking_reward_token_mint,
            token_program,
            perpetuals_program: perpetuals_program.clone(),
        };

        let cpi_program = perpetuals_program;
        let cpi_context = anchor_lang::context::CpiContext::new(cpi_program, cpi_accounts)
            .with_signer(authority_seeds);

        crate::cpi::swap(cpi_context, params)
    }

    /// The governance is managed through the program only.
    /// On behalf of users, the program manages their voting power (through Vest and Stake they own).
    /// Depending of the lm_token contained in these accounts and of their voting multiplier, if any, the
    /// program mint new governance token that are own by said Stake/Vest accounts and their voting power are
    /// delegated to the owner (the end user).
    /// This allow flexible voting power with multiplier, decorrelated from the actual lm_token amount held in these
    /// accounts.
    /// Furthermore, this enforces that the governance token is soulbound to a user, non tradable.
    ///
    /// Updated: Governance is setup with Membership, which allow us to set the owner as the final owner and
    /// avoid delegation of vote (simplify things).
    /// Owner can auto revoke at worse, and to hedge against this we always revoke the min amount between
    /// user voting power and our initial revoke target.
    #[allow(clippy::too_many_arguments)]
    pub fn remove_governing_power<'a>(
        &self,
        transfer_authority: AccountInfo<'a>,
        // the owner of the voting power that will be delegated. (a PDA like Vest or Stake)
        governing_token_owner: AccountInfo<'a>,
        governing_token_owner_record: AccountInfo<'a>,
        // mint of the shadow governance token (will burn)
        governance_token_mint: AccountInfo<'a>,
        realm: AccountInfo<'a>,
        realm_config: AccountInfo<'a>,
        governing_token_holding: AccountInfo<'a>,
        governance_program: AccountInfo<'a>,
        amount: u64,
    ) -> Result<()> {
        let token_owner_record_data = get_token_owner_record_data(
            governance_program.key,
            governing_token_owner_record.to_account_info().as_ref(),
        )?;
        msg!("ok");

        // Calculate the min amount between target revocation and the amount held by user. This is to prevent issues
        // in the scenario where the user self revoke some token (which is possible through the gov)
        let revoke_amount = min(
            amount,
            token_owner_record_data.governing_token_deposit_amount,
        );
        msg!(
            "Governance - Revoke {} (target: {}) governing power from the owner: {}",
            revoke_amount,
            amount,
            governing_token_owner.key
        );

        // Revoke tokens (the owner (vest or stake) get burnt the revoked amount of token)
        {
            let authority_seeds: &[&[&[u8]]] =
                &[&[b"transfer_authority", &[self.transfer_authority_bump]]];

            let cpi_accounts = adapters::RevokeGoverningTokens {
                realm: realm.to_account_info(),
                governing_token_holding,
                governing_token_owner_record: governing_token_owner_record.to_account_info(),
                governing_token_mint: governance_token_mint.to_account_info(),
                governing_token_revoke_authority: transfer_authority.to_account_info(),
                realm_config,
                governing_token_owner: governing_token_owner.to_account_info(),
                governing_token_mint_authority: transfer_authority.to_account_info(),
            };

            let cpi_program = governance_program.to_account_info();

            adapters::revoke_governing_token(
                CpiContext::new(cpi_program, cpi_accounts).with_signer(authority_seeds),
                revoke_amount,
            )?;
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_governing_power<'a>(
        &self,
        transfer_authority: AccountInfo<'a>,
        payer: AccountInfo<'a>,
        governing_token_owner: AccountInfo<'a>,
        governing_token_owner_record: AccountInfo<'a>,
        // mint of the shadow governance token (will mint)
        governance_token_mint: AccountInfo<'a>,
        realm: AccountInfo<'a>,
        realm_config: AccountInfo<'a>,
        governing_token_holding: AccountInfo<'a>,
        governance_program: AccountInfo<'a>,
        amount: u64,
        additional_signer_seeds: Option<&[&[u8]]>,
        owner_is_signer: bool,
    ) -> Result<()> {
        msg!(
            "Governance - Mint {} governing power to the owner: {}",
            amount,
            governing_token_owner.key
        );
        // Mint tokens in governance for the owner
        {
            let authority_seeds: &[&[u8]] =
                &[b"transfer_authority", &[self.transfer_authority_bump]];

            let cpi_accounts = adapters::DepositGoverningTokens {
                realm: realm.to_account_info(),
                governing_token_mint: governance_token_mint.to_account_info(),
                governing_token_source: governance_token_mint.to_account_info(),
                governing_token_owner: governing_token_owner.to_account_info(),
                governing_token_transfer_authority: transfer_authority,
                payer,
                realm_config,
                governing_token_holding,
                governing_token_owner_record: governing_token_owner_record.to_account_info(),
            };

            // In case the owner is not signer in involved TX (addVest for instance)
            let signers_seeds = match additional_signer_seeds {
                Some(additional_signer_seeds) => vec![authority_seeds, additional_signer_seeds],
                None => vec![authority_seeds],
            };

            let cpi_program = governance_program.to_account_info();
            match owner_is_signer {
                true => adapters::deposit_governing_tokens(
                    CpiContext::new(cpi_program, cpi_accounts).with_signer(&signers_seeds),
                    amount,
                )?,
                false => adapters::deposit_governing_tokens_owner_not_signer(
                    CpiContext::new(cpi_program, cpi_accounts).with_signer(&signers_seeds),
                    amount,
                )?,
            }
        }

        Ok(())
    }
}
