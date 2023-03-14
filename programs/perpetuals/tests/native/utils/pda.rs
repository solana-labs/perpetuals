use {
    perpetuals::{adapters::spl_governance_program_adapter, state::position::Side},
    solana_sdk::pubkey::Pubkey,
};

pub fn get_multisig_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&["multisig".as_ref()], &perpetuals::id())
}

pub fn get_transfer_authority_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&["transfer_authority".as_ref()], &perpetuals::id())
}

pub fn get_cortex_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&["cortex".as_ref()], &perpetuals::id())
}

pub fn get_perpetuals_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&["perpetuals".as_ref()], &perpetuals::id())
}

pub fn get_lm_token_mint_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&["lm_token_mint".as_ref()], &perpetuals::id())
}

pub fn get_vest_token_account_pda(vest_pda: Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &["vest_token_account".as_ref(), vest_pda.as_ref()],
        &perpetuals::id(),
    )
}

pub fn get_stake_token_account_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&["stake_token_account".as_ref()], &perpetuals::id())
}

pub fn get_stake_redeemable_token_mint_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&["stake_redeemable_token_mint".as_ref()], &perpetuals::id())
}

pub fn get_program_data_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[perpetuals::id().as_ref()],
        &solana_program::bpf_loader_upgradeable::id(),
    )
}

pub fn get_pool_pda(name: String) -> (Pubkey, u8) {
    Pubkey::find_program_address(&["pool".as_ref(), name.as_bytes()], &perpetuals::id())
}

pub fn get_vest_pda(owner: Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&["vest".as_ref(), owner.as_ref()], &perpetuals::id())
}

pub fn get_lp_token_mint_pda(pool_pda: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &["lp_token_mint".as_ref(), pool_pda.as_ref()],
        &perpetuals::id(),
    )
}

pub fn get_custody_pda(pool_pda: &Pubkey, custody_token_mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            "custody".as_ref(),
            pool_pda.as_ref(),
            custody_token_mint.as_ref(),
        ],
        &perpetuals::id(),
    )
}

pub fn get_position_pda(
    owner: &Pubkey,
    pool_pda: &Pubkey,
    custody_pda: &Pubkey,
    side: Side,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            "position".as_ref(),
            owner.as_ref(),
            pool_pda.as_ref(),
            custody_pda.as_ref(),
            &[side as u8],
        ],
        &perpetuals::id(),
    )
}

pub fn get_custody_token_account_pda(
    pool_pda: &Pubkey,
    custody_token_mint: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            "custody_token_account".as_ref(),
            pool_pda.as_ref(),
            custody_token_mint.as_ref(),
        ],
        &perpetuals::id(),
    )
}

pub fn get_test_oracle_account(pool_pda: &Pubkey, custody_mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            "oracle_account".as_ref(),
            pool_pda.as_ref(),
            custody_mint.as_ref(),
        ],
        &perpetuals::id(),
    )
}

pub fn get_governance_realm_pda(name: String) -> Pubkey {
    spl_governance::state::realm::get_realm_address(&spl_governance_program_adapter::ID, &name)
}

pub fn get_governance_governing_token_holding_pda(
    governance_realm_pda: &Pubkey,
    governing_token_mint: &Pubkey,
) -> Pubkey {
    spl_governance::state::realm::get_governing_token_holding_address(
        &spl_governance_program_adapter::ID,
        governance_realm_pda,
        governing_token_mint,
    )
}

pub fn get_governance_realm_config_pda(governance_realm_pda: &Pubkey) -> Pubkey {
    spl_governance::state::realm_config::get_realm_config_address(
        &spl_governance_program_adapter::ID,
        governance_realm_pda,
    )
}

pub fn get_governance_governing_token_owner_record_pda(
    governance_realm_pda: &Pubkey,
    governing_token_mint: &Pubkey,
    governing_token_owner: &Pubkey,
) -> Pubkey {
    spl_governance::state::token_owner_record::get_token_owner_record_address(
        &spl_governance_program_adapter::ID,
        governance_realm_pda,
        governing_token_mint,
        governing_token_owner,
    )
}
