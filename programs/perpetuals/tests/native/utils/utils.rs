use {
    super::get_program_data_pda,
    crate::instructions,
    anchor_lang::{prelude::*, InstructionData},
    anchor_spl::token::spl_token,
    bonfida_test_utils::ProgramTestContextExt,
    borsh::BorshDeserialize,
    perpetuals::{
        instructions::SetCustodyConfigParams,
        math,
        state::{custody::Custody, perpetuals::Perpetuals, pool::TokenRatios},
    },
    solana_program::{
        bpf_loader_upgradeable, clock::SLOT_MS, epoch_schedule::DEFAULT_SLOTS_PER_EPOCH,
        program_pack::Pack, stake_history::Epoch,
    },
    solana_program_test::{read_file, BanksClientError, ProgramTest, ProgramTestContext},
    solana_sdk::{account, signature::Keypair, signer::Signer, signers::Signers},
    std::{
        ops::{Div, Mul},
        path::Path,
    },
};

pub const ANCHOR_DISCRIMINATOR_SIZE: usize = 8;

pub fn create_and_fund_account(address: &Pubkey, program_test: &mut ProgramTest) {
    program_test.add_account(
        *address,
        account::Account {
            lamports: 1_000_000_000,
            ..account::Account::default()
        },
    );
}

pub fn find_associated_token_account(owner: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            owner.as_ref(),
            anchor_spl::token::ID.as_ref(),
            mint.as_ref(),
        ],
        &anchor_spl::associated_token::ID,
    )
}

pub fn copy_keypair(keypair: &Keypair) -> Keypair {
    Keypair::from_bytes(&keypair.to_bytes()).unwrap()
}

pub async fn get_token_account(
    program_test_ctx: &mut ProgramTestContext,
    key: Pubkey,
) -> spl_token::state::Account {
    let raw_account = program_test_ctx
        .banks_client
        .get_account(key)
        .await
        .unwrap()
        .unwrap();

    spl_token::state::Account::unpack(&raw_account.data).unwrap()
}

pub async fn get_token_account_balance(
    program_test_ctx: &mut ProgramTestContext,
    key: Pubkey,
) -> u64 {
    get_token_account(program_test_ctx, key).await.amount
}

pub async fn get_account<T: anchor_lang::AccountDeserialize>(
    program_test_ctx: &mut ProgramTestContext,
    key: Pubkey,
) -> T {
    let account = program_test_ctx
        .banks_client
        .get_account(key)
        .await
        .unwrap()
        .unwrap();

    T::try_deserialize(&mut account.data.as_slice()).unwrap()
}

pub async fn get_current_unix_timestamp(program_test_ctx: &mut ProgramTestContext) -> i64 {
    program_test_ctx
        .banks_client
        .get_sysvar::<solana_program::sysvar::clock::Clock>()
        .await
        .unwrap()
        .unix_timestamp
}

pub async fn initialize_token_account(
    program_test_ctx: &mut ProgramTestContext,
    mint: &Pubkey,
    owner: &Pubkey,
) -> Pubkey {
    program_test_ctx
        .initialize_token_accounts(*mint, &[*owner])
        .await
        .unwrap()[0]
}

pub async fn initialize_and_fund_token_account(
    program_test_ctx: &mut ProgramTestContext,
    mint: &Pubkey,
    owner: &Pubkey,
    mint_authority: &Keypair,
    amount: u64,
) -> Pubkey {
    let token_account_address = initialize_token_account(program_test_ctx, mint, owner).await;

    mint_tokens(
        program_test_ctx,
        mint_authority,
        mint,
        &token_account_address,
        amount,
    )
    .await;

    token_account_address
}

pub async fn mint_tokens(
    program_test_ctx: &mut ProgramTestContext,
    mint_authority: &Keypair,
    mint: &Pubkey,
    token_account: &Pubkey,
    amount: u64,
) {
    program_test_ctx
        .mint_tokens(mint_authority, mint, token_account, amount)
        .await
        .unwrap();
}

// Deploy the perpetuals program onchain as upgradeable program
pub async fn add_perpetuals_program(program_test: &mut ProgramTest, upgrade_authority: &Keypair) {
    // Deploy two accounts, one describing the program
    // and a second one holding the program's binary bytes
    let mut program_bytes = read_file(
        std::env::current_dir()
            .unwrap()
            .join(Path::new("../../target/deploy/perpetuals.so")),
    );

    let program_data_pda = get_program_data_pda().0;

    let program = UpgradeableLoaderState::Program {
        programdata_address: program_data_pda,
    };
    let program_data = UpgradeableLoaderState::ProgramData {
        slot: 1,
        upgrade_authority_address: Some(upgrade_authority.pubkey()),
    };

    let serialized_program = bincode::serialize(&program).unwrap();

    let mut serialzed_program_data = bincode::serialize(&program_data).unwrap();
    serialzed_program_data.append(&mut program_bytes);

    let program_account = account::Account {
        lamports: Rent::default().minimum_balance(serialized_program.len()),
        data: serialized_program,
        owner: bpf_loader_upgradeable::ID,
        executable: true,
        rent_epoch: Epoch::default(),
    };
    let program_data_account = account::Account {
        lamports: Rent::default().minimum_balance(serialzed_program_data.len()),
        data: serialzed_program_data,
        owner: bpf_loader_upgradeable::ID,
        executable: false,
        rent_epoch: Epoch::default(),
    };

    program_test.add_account(perpetuals::id(), program_account);
    program_test.add_account(program_data_pda, program_data_account);
}

pub async fn create_and_fund_multiple_accounts(
    program_test: &mut ProgramTest,
    number: usize,
) -> Vec<Keypair> {
    let mut keypairs = Vec::new();

    for _ in 0..number {
        keypairs.push(Keypair::new());
    }

    keypairs
        .iter()
        .for_each(|k| create_and_fund_account(&k.pubkey(), program_test));

    keypairs
}

pub async fn create_and_simulate_perpetuals_view_ix<T: InstructionData, U: BorshDeserialize>(
    program_test_ctx: &mut ProgramTestContext,
    accounts_meta: Vec<AccountMeta>,
    args: T,
    payer: &Keypair,
) -> std::result::Result<U, BanksClientError> {
    let ix = solana_sdk::instruction::Instruction {
        program_id: perpetuals::id(),
        accounts: accounts_meta,
        data: args.data(),
    };

    let payer_pubkey = payer.pubkey();

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer_pubkey),
        &[payer],
        program_test_ctx.last_blockhash,
    );

    let result = program_test_ctx.banks_client.simulate_transaction(tx).await;

    if result.is_err() {
        return Err(result.err().unwrap());
    }

    // Extract the returned data
    let mut return_data: Vec<u8> = result
        .unwrap()
        .simulation_details
        .unwrap()
        .return_data
        .unwrap()
        .data;

    let result_expected_len = std::mem::size_of::<U>();

    // Returned data doesn't contains leading zeros, need to re-add them before deserialization
    while return_data.len() < result_expected_len {
        return_data.push(0u8);
    }

    Ok(U::try_from_slice(return_data.as_slice()).unwrap())
}

pub async fn create_and_execute_perpetuals_ix<T: InstructionData, U: Signers>(
    program_test_ctx: &mut ProgramTestContext,
    accounts_meta: Vec<AccountMeta>,
    args: T,
    payer: Option<&Pubkey>,
    signing_keypairs: &U,
) -> std::result::Result<(), BanksClientError> {
    let ix = solana_sdk::instruction::Instruction {
        program_id: perpetuals::id(),
        accounts: accounts_meta,
        data: args.data(),
    };

    let tx = solana_sdk::transaction::Transaction::new_signed_with_payer(
        &[ix],
        payer,
        signing_keypairs,
        program_test_ctx.last_blockhash,
    );

    let result = program_test_ctx.banks_client.process_transaction(tx).await;

    if result.is_err() {
        return Err(result.err().unwrap());
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn set_custody_ratios(
    program_test_ctx: &mut ProgramTestContext,
    custody_admin: &Keypair,
    payer: &Keypair,
    custody_pda: &Pubkey,
    ratios: Vec<TokenRatios>,
    multisig_signers: &[&Keypair],
) {
    let custody_account = get_account::<Custody>(program_test_ctx, *custody_pda).await;

    instructions::test_set_custody_config(
        program_test_ctx,
        custody_admin,
        payer,
        &custody_account.pool,
        custody_pda,
        SetCustodyConfigParams {
            is_stable: custody_account.is_stable,
            is_virtual: custody_account.is_virtual,
            oracle: custody_account.oracle,
            pricing: custody_account.pricing,
            permissions: custody_account.permissions,
            fees: custody_account.fees,
            borrow_rate: custody_account.borrow_rate,
            ratios,
        },
        multisig_signers,
    )
    .await
    .unwrap();
}

#[derive(Clone, Copy)]
pub struct SetupCustodyInfo {
    pub test_oracle_pda: Pubkey,
    pub custody_pda: Pubkey,
}

pub fn scale(amount: u64, decimals: u8) -> u64 {
    math::checked_mul(amount, 10u64.pow(decimals as u32)).unwrap()
}

pub fn scale_f64(amount: f64, decimals: u8) -> u64 {
    math::checked_as_u64(
        math::checked_float_mul(amount, 10u64.pow(decimals as u32) as f64).unwrap(),
    )
    .unwrap()
}

pub fn ratio_from_percentage(percentage: f64) -> u64 {
    (Perpetuals::BPS_POWER as f64)
        .mul(percentage)
        .div(100_f64)
        .floor() as u64
}

pub async fn initialize_users_token_accounts(
    program_test_ctx: &mut ProgramTestContext,
    mints: Vec<Pubkey>,
    users: Vec<Pubkey>,
) {
    for mint in mints {
        program_test_ctx
            .initialize_token_accounts(mint, users.as_slice())
            .await
            .unwrap();
    }
}

// Doesn't check if you go before epoch 0 when passing negative amounts, be wary
pub async fn warp_forward(ctx: &mut ProgramTestContext, seconds: i64) {
    let clock_sysvar: Clock = ctx.banks_client.get_sysvar().await.unwrap();
    println!(
        "Original Time: epoch = {}, timestamp = {}",
        clock_sysvar.epoch, clock_sysvar.unix_timestamp
    );
    let mut new_clock = clock_sysvar.clone();
    new_clock.unix_timestamp += seconds;

    let seconds_since_epoch_start = new_clock.unix_timestamp - clock_sysvar.epoch_start_timestamp;
    let ms_since_epoch_start = seconds_since_epoch_start * 1_000;
    let slots_since_epoch_start = ms_since_epoch_start / SLOT_MS as i64;
    let epochs_since_epoch_start = slots_since_epoch_start / DEFAULT_SLOTS_PER_EPOCH as i64;
    new_clock.epoch = (new_clock.epoch as i64 + epochs_since_epoch_start) as u64;

    ctx.set_sysvar(&new_clock);
    let clock_sysvar: Clock = ctx.banks_client.get_sysvar().await.unwrap();
    println!(
        "New Time: epoch = {}, timestamp = {}",
        clock_sysvar.epoch, clock_sysvar.unix_timestamp
    );

    ctx.last_blockhash = ctx.banks_client.get_latest_blockhash().await.unwrap();
}
