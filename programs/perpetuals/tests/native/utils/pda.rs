use {
    perpetuals::{
        adapters::spl_governance_program_adapter,
        state::{position::Side, user_staking::USER_STAKING_THREAD_AUTHORITY_SEED},
    },
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

pub fn get_clockwork_thread_pda(thread_authority: &Pubkey, thread_id: Vec<u8>) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            clockwork_thread_program::state::SEED_THREAD,
            thread_authority.as_ref(),
            thread_id.as_slice(),
        ],
        &clockwork_thread_program::id(),
    )
}

pub fn get_clockwork_network_config_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[clockwork_network_program::state::SEED_CONFIG],
        &clockwork_network_program::id(),
    )
}

pub fn get_clockwork_network_registry_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[clockwork_network_program::state::SEED_REGISTRY],
        &clockwork_network_program::id(),
    )
}

pub fn get_clockwork_network_snapshot_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            clockwork_network_program::state::SEED_SNAPSHOT,
            (0_u64).to_be_bytes().as_ref(),
        ],
        &clockwork_network_program::id(),
    )
}

pub fn get_clockwork_network_fee_pda(worker: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[clockwork_network_program::state::SEED_FEE, worker.as_ref()],
        &clockwork_network_program::id(),
    )
}

pub fn get_clockwork_network_penalty_pda(worker: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            clockwork_network_program::state::SEED_PENALTY,
            worker.as_ref(),
        ],
        &clockwork_network_program::id(),
    )
}

pub fn get_clockwork_network_worker_pda(index: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            clockwork_network_program::state::SEED_WORKER,
            index.to_be_bytes().as_ref(),
        ],
        &clockwork_network_program::id(),
    )
}

pub fn get_governance_token_mint_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&["governance_token_mint".as_ref()], &perpetuals::id())
}

pub fn get_user_staking_pda(owner: &Pubkey, staking_pda: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            "user_staking".as_ref(),
            owner.as_ref(),
            staking_pda.as_ref(),
        ],
        &perpetuals::id(),
    )
}

pub fn get_staking_pda(staked_token_mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &["staking".as_ref(), staked_token_mint.as_ref()],
        &perpetuals::id(),
    )
}

pub fn get_thread_address(user_staking_thread_authority: &Pubkey, thread_id: Vec<u8>) -> Pubkey {
    clockwork_sdk::state::Thread::pubkey(*user_staking_thread_authority, thread_id)
}

pub fn get_user_staking_thread_authority(user_staking_pda: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            USER_STAKING_THREAD_AUTHORITY_SEED,
            user_staking_pda.as_ref(),
        ],
        &perpetuals::id(),
    )
}

pub fn get_staking_staked_token_vault_pda(staking_pda: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &["staking_staked_token_vault".as_ref(), staking_pda.as_ref()],
        &perpetuals::id(),
    )
}

pub fn get_staking_reward_token_vault_pda(staking_pda: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &["staking_reward_token_vault".as_ref(), staking_pda.as_ref()],
        &perpetuals::id(),
    )
}

pub fn get_staking_lm_reward_token_vault_pda(staking_pda: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            "staking_lm_reward_token_vault".as_ref(),
            staking_pda.as_ref(),
        ],
        &perpetuals::id(),
    )
}

pub fn get_program_data_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[perpetuals::id().as_ref()],
        &solana_program::bpf_loader_upgradeable::id(),
    )
}

pub fn get_pool_pda(name: &String) -> (Pubkey, u8) {
    Pubkey::find_program_address(&["pool".as_ref(), name.as_bytes()], &perpetuals::id())
}

pub fn get_vest_pda(owner: &Pubkey) -> (Pubkey, u8) {
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

pub fn get_custom_oracle_account(pool_pda: &Pubkey, custody_mint: &Pubkey) -> (Pubkey, u8) {
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
