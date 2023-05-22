use {crate::adapters, anchor_lang::prelude::*, solana_program::account_info::AccountInfo};

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
pub fn remove_governing_power<'a>(
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
    transfer_authority_seeds: &[&[u8]],
    governing_token_owner_seeds: &[&[u8]],
    // the amount of votes to revoke
    amount: u64,
    // the votes owner
    governing_power_recipient: AccountInfo<'a>,
) -> Result<()> {
    // Revoke vote delegation (claw back any voting power)
    {
        let cpi_accounts = adapters::SetGovernanceDelegate {
            governance_authority: governing_token_owner.to_account_info(),
            realm: realm.to_account_info(),
            governing_token_mint: governance_token_mint.to_account_info(),
            governing_token_owner: governing_token_owner.to_account_info(),
            governing_token_owner_record: governing_token_owner_record.to_account_info(),
        };

        let cpi_program = governance_program.to_account_info();

        adapters::set_governance_delegate(
            CpiContext::new(cpi_program, cpi_accounts)
                .with_signer(&[transfer_authority_seeds, governing_token_owner_seeds]),
        )?;
    }

    // Revoke tokens (the owner (vest or stake) get burnt the revoked amount of token)
    {
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
            CpiContext::new(cpi_program, cpi_accounts)
                .with_signer(&[transfer_authority_seeds, governing_token_owner_seeds]),
            amount,
        )?;
    }

    // Re delegate voting power to the end user (if any)
    {
        let cpi_accounts = adapters::SetGovernanceDelegate {
            governance_authority: governing_token_owner.to_account_info(),
            realm,
            governing_token_mint: governance_token_mint,
            governing_token_owner,
            governing_token_owner_record,
        };

        let cpi_program = governance_program;

        let mut cpi_context = CpiContext::new(cpi_program, cpi_accounts);

        cpi_context
            .remaining_accounts
            .append(&mut Vec::from([governing_power_recipient]));

        adapters::set_governance_delegate(
            cpi_context.with_signer(&[transfer_authority_seeds, governing_token_owner_seeds]),
        )?;
    }

    Ok(())
}

pub fn add_governing_power<'a>(
    transfer_authority: AccountInfo<'a>,
    payer: AccountInfo<'a>,
    // the owner of the voting power that will be delegated. (a PDA like Vest or Stake)
    governing_token_owner: AccountInfo<'a>,
    governing_token_owner_record: AccountInfo<'a>,
    // mint of the shadow governance token (will mint)
    governance_token_mint: AccountInfo<'a>,
    realm: AccountInfo<'a>,
    realm_config: AccountInfo<'a>,
    governing_token_holding: AccountInfo<'a>,
    governance_program: AccountInfo<'a>,
    transfer_authority_seeds: &[&[u8]],
    governing_token_owner_seeds: &[&[u8]],
    // the amount of voting power to add
    amount: u64,
    // the delegation target (the user given the voting power)
    governing_power_recipient: AccountInfo<'a>,
) -> Result<()> {
    // Mint tokens in governance for the owner
    {
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

        let cpi_program = governance_program.to_account_info();

        adapters::deposit_governing_tokens(
            CpiContext::new(cpi_program, cpi_accounts)
                .with_signer(&[transfer_authority_seeds, governing_token_owner_seeds]),
            amount,
        )?;
    }
    // Delegate owner power to the end user (governing_power_recipient)
    {
        let cpi_accounts = adapters::SetGovernanceDelegate {
            governance_authority: governing_token_owner.to_account_info(),
            realm,
            governing_token_mint: governance_token_mint,
            governing_token_owner,
            governing_token_owner_record,
        };

        let cpi_program = governance_program;

        let mut cpi_context = CpiContext::new(cpi_program, cpi_accounts);

        cpi_context
            .remaining_accounts
            .append(&mut Vec::from([governing_power_recipient]));

        adapters::set_governance_delegate(
            cpi_context.with_signer(&[transfer_authority_seeds, governing_token_owner_seeds]),
        )?;
    }

    Ok(())
}
