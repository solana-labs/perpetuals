use {
    crate::instructions::SwapParams,
    anchor_lang::prelude::*,
    anchor_spl::token::{Burn, MintTo, Transfer},
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
    pub const BPS_POWER: u128 = 10i64.pow(Self::BPS_DECIMALS as u32) as u128;
    pub const PRICE_DECIMALS: u8 = 6;
    pub const USD_DECIMALS: u8 = 6;
    pub const LP_DECIMALS: u8 = Self::USD_DECIMALS;
    pub const RATE_DECIMALS: u8 = 9;
    pub const RATE_POWER: u128 = 10i64.pow(Self::RATE_DECIMALS as u32) as u128;

    pub fn validate(&self) -> bool {
        true
    }

    // REPLACE WITH warp to slot
    #[cfg(feature = "test")]
    pub fn get_time(&self) -> Result<i64> {
        Ok(self.inception_time)
    }

    #[cfg(not(feature = "test"))]
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

    // recursive swap CPI
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
        stake_reward_token_custody: AccountInfo<'a>,
        stake_reward_token_custody_oracle_account: AccountInfo<'a>,
        stake_reward_token_custody_token_account: AccountInfo<'a>,
        stake_reward_token_account: AccountInfo<'a>,
        stake_reward_token_mint: AccountInfo<'a>,
        lm_token_mint: AccountInfo<'a>,
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
            pool,
            receiving_custody,
            receiving_custody_oracle_account,
            receiving_custody_token_account,
            dispensing_custody,
            dispensing_custody_oracle_account,
            dispensing_custody_token_account,
            stake_reward_token_custody,
            stake_reward_token_custody_oracle_account,
            stake_reward_token_custody_token_account,
            stake_reward_token_account,
            lm_token_mint,
            stake_reward_token_mint,
            token_program,
            perpetuals_program: perpetuals_program.clone(),
        };
        let cpi_program = perpetuals_program;
        let cpi_context = anchor_lang::context::CpiContext::new(cpi_program, cpi_accounts)
            .with_signer(authority_seeds);

        crate::cpi::swap(cpi_context, params)
    }
}
