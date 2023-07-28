use {
    super::{pda, SetupCustodyInfo},
    crate::{
        adapters, test_instructions,
        utils::{self, fixtures},
    },
    bonfida_test_utils::ProgramTestExt,
    perpetuals::{
        instructions::{
            AddCustodyParams, AddLiquidityParams, InitStakingParams, SetCustomOraclePriceParams,
        },
        state::{
            custody::{BorrowRateParams, Fees, PricingParams},
            perpetuals::Permissions,
            pool::TokenRatios,
            staking::StakingType,
        },
    },
    solana_program::pubkey::Pubkey,
    solana_program_test::{ProgramTest, ProgramTestContext},
    solana_sdk::{signature::Keypair, signer::Signer},
    std::collections::HashMap,
    tokio::sync::RwLock,
};

pub struct SetupCustodyWithLiquidityParams<'a> {
    pub setup_custody_params: SetupCustodyParams<'a>,
    pub liquidity_amount: u64,

    // Who's adding the liquidity?
    pub payer_user_name: &'a str,
}

pub struct SetupCustodyParams<'a> {
    // Which mint is it about
    pub mint_name: &'a str,

    pub is_stable: bool,
    pub is_virtual: bool,
    pub target_ratio: u64,
    pub min_ratio: u64,
    pub max_ratio: u64,
    pub initial_price: u64,
    pub initial_conf: u64,
    pub pricing_params: Option<PricingParams>,
    pub permissions: Option<Permissions>,
    pub fees: Option<Fees>,
    pub borrow_rate: Option<BorrowRateParams>,
}

pub struct UserParam<'a> {
    pub name: &'a str,

    // mint_name: amount
    pub token_balances: HashMap<&'a str, u64>,
}

pub struct MintParam<'a> {
    pub name: &'a str,
    pub decimals: u8,
}

pub struct MintInfo {
    pub decimals: u8,
    pub pubkey: Pubkey,
}

pub struct TestSetup {
    pub program_test_ctx: RwLock<ProgramTestContext>,

    pub root_authority_keypair: Keypair,
    pub payer_keypair: Keypair,

    pub users: HashMap<String, Keypair>,
    pub mints: HashMap<String, MintInfo>,
    pub multisig_members: HashMap<String, Keypair>,

    pub cortex_stake_reward_mint_name: String,

    pub governance_realm_pda: Pubkey,
    pub gov_token_mint_pda: Pubkey,
    pub lm_token_mint: Pubkey,

    pub pool_pda: Pubkey,
    pub pool_bump: u8,
    pub lp_token_mint_pda: Pubkey,
    pub lp_token_mint_bump: u8,
    pub custodies_info: Vec<SetupCustodyInfo>,

    pub clockwork_signatory: Keypair,
    pub clockwork_mint_reward_name: String,
}

impl TestSetup {
    // Only use one worker in the tests
    pub fn get_clockwork_worker(&self) -> Pubkey {
        pda::get_clockwork_network_worker_pda(0).0
    }

    pub fn get_user_keypair_by_name(&self, name: &str) -> &Keypair {
        self.users.get(&name.to_string()).unwrap()
    }

    pub fn get_multisig_member_keypair_by_name(&self, name: &str) -> &Keypair {
        self.multisig_members.get(&name.to_string()).unwrap()
    }

    pub fn get_multisig_signers(&self) -> Vec<&Keypair> {
        self.multisig_members.values().collect()
    }

    pub fn get_cortex_stake_reward_mint(&self) -> Pubkey {
        self.mints
            .get(&self.cortex_stake_reward_mint_name)
            .unwrap()
            .pubkey
    }

    pub fn get_mint_by_name(&self, name: &str) -> Pubkey {
        self.mints.get(&name.to_string()).unwrap().pubkey
    }

    // Initialize everything required to test the program
    // Create the mints, the users, deploy the program, create the pool and the custodies, provide liquidity.
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        users_param: Vec<UserParam<'_>>,
        mints_param: Vec<MintParam<'_>>,
        multisig_members_names: Vec<&str>,
        // mint for the payouts of the LM token staking (ADX staking)
        cortex_stake_reward_mint_name: &str,
        // mint used by clockwork to distribute rewards
        clockwork_mint_reward_name: &str,
        governance_token_decimals: u8,
        governance_realm_name: &str,
        pool_name: &str,
        custodies_params: Vec<SetupCustodyWithLiquidityParams<'_>>,
        core_contributor_bucket_allocation: u64,
        dao_treasury_bucket_allocation: u64,
        pol_bucket_allocation: u64,
        ecosystem_bucket_allocation: u64,
    ) -> TestSetup {
        let mut program_test = ProgramTest::default();

        // Initialize keypairs
        let keypairs: Vec<Keypair> = utils::create_and_fund_multiple_accounts(
            &mut program_test,
            // 1 keypair per user
            users_param.len() +
            // payer
            1 +
            // clockwork signatory
            1+
            // root authority
            1 +
            // program upgrade authority
            1 +
            // 1 keypair per multisig member
            multisig_members_names.len(),
        )
        .await;

        // Name keypairs
        let (
            users_keypairs,
            payer_keypair,
            clockwork_signatory,
            root_authority_keypair,
            program_authority_keypair,
            multisig_members_keypairs,
        ) = {
            (
                &keypairs[0..users_param.len()],
                keypairs.get(users_param.len()).unwrap(),
                keypairs.get(users_param.len() + 1).unwrap(),
                keypairs.get(users_param.len() + 2).unwrap(),
                keypairs.get(users_param.len() + 3).unwrap(),
                &keypairs
                    [users_param.len() + 4..(users_param.len() + 4 + multisig_members_names.len())],
            )
        };

        let users = {
            let mut users: HashMap<String, Keypair> = HashMap::new();
            for (i, user_param) in users_param.as_slice().iter().enumerate() {
                users.insert(
                    user_param.name.to_string(),
                    utils::copy_keypair(&users_keypairs[i]),
                );
            }

            users
        };

        // Initialize mints
        let mints = {
            let mut mints: HashMap<String, MintInfo> = HashMap::new();

            for mint_param in mints_param {
                let mint_pubkey = program_test
                    .add_mint(None, mint_param.decimals, &root_authority_keypair.pubkey())
                    .0;

                mints.insert(
                    mint_param.name.to_string(),
                    MintInfo {
                        decimals: mint_param.decimals,
                        pubkey: mint_pubkey,
                    },
                );
            }

            mints
        };

        // Deploy programs
        {
            utils::add_perpetuals_program(&mut program_test, program_authority_keypair).await;
            utils::add_spl_governance_program(&mut program_test, program_authority_keypair).await;
            utils::add_clockwork_network_program(&mut program_test, program_authority_keypair)
                .await;
            utils::add_clockwork_thread_program(&mut program_test, program_authority_keypair).await;
        }

        // Start the client and connect to localnet validator
        let program_test_ctx: RwLock<ProgramTestContext> =
            RwLock::new(program_test.start_with_context().await);

        // Initialize multisig
        let multisig_members = {
            let mut multisig_members: HashMap<String, Keypair> = HashMap::new();
            for (i, multisig_member_name) in multisig_members_names.into_iter().enumerate() {
                multisig_members.insert(
                    multisig_member_name.to_string(),
                    utils::copy_keypair(&multisig_members_keypairs[i]),
                );
            }

            multisig_members
        };

        let multisig_signers: Vec<&Keypair> = multisig_members.values().collect();

        let staking_reward_token_mint = &mints.get(cortex_stake_reward_mint_name).unwrap().pubkey;

        let governance_realm_pda = pda::get_governance_realm_pda(governance_realm_name.to_string());
        let gov_token_mint_pda = pda::get_governance_token_mint_pda().0;

        // Execute the initialize transaction
        test_instructions::init(
            &program_test_ctx,
            program_authority_keypair,
            fixtures::init_params_permissions_full(
                1,
                core_contributor_bucket_allocation,
                dao_treasury_bucket_allocation,
                pol_bucket_allocation,
                ecosystem_bucket_allocation,
            ),
            &governance_realm_pda,
            staking_reward_token_mint,
            &multisig_signers,
        )
        .await
        .unwrap();

        // Setup the governance
        {
            adapters::spl_governance::create_realm(
                &program_test_ctx,
                root_authority_keypair,
                payer_keypair,
                governance_realm_name.to_string(),
                utils::scale(10_000, governance_token_decimals),
                &gov_token_mint_pda,
            )
            .await
            .unwrap();
        }

        // Setup clockwork
        {
            adapters::clockwork::network::initialize(
                &program_test_ctx,
                root_authority_keypair,
                payer_keypair,
                &mints[clockwork_mint_reward_name].pubkey,
            )
            .await
            .unwrap();

            adapters::clockwork::network::pool_create(
                &program_test_ctx,
                root_authority_keypair,
                payer_keypair,
            )
            .await
            .unwrap();

            adapters::clockwork::network::worker_create(
                &program_test_ctx,
                root_authority_keypair,
                clockwork_signatory,
                payer_keypair,
                &mints[clockwork_mint_reward_name].pubkey,
            )
            .await
            .unwrap();
        }

        let lm_token_mint = utils::pda::get_lm_token_mint_pda().0;

        // Initialize users token accounts for each mints
        {
            let mut mints_pubkeys: Vec<Pubkey> =
                mints.values().into_iter().map(|info| info.pubkey).collect();

            mints_pubkeys.push(lm_token_mint);

            let users_pubkeys: Vec<Pubkey> = users
                .values()
                .into_iter()
                .map(|keypair| keypair.pubkey())
                .collect();

            utils::initialize_users_token_accounts(&program_test_ctx, mints_pubkeys, users_pubkeys)
                .await;
        }

        // Mint tokens for users to match specified balances
        {
            for user_param in users_param.as_slice() {
                for (mint_name, amount) in &user_param.token_balances {
                    let mint = mints.get(&mint_name.to_string()).unwrap().pubkey;
                    let user = users.get(&user_param.name.to_string()).unwrap().pubkey();

                    let ata = utils::find_associated_token_account(&user, &mint).0;

                    utils::mint_tokens(
                        &program_test_ctx,
                        root_authority_keypair,
                        &mint,
                        &ata,
                        *amount,
                    )
                    .await;
                }
            }
        }

        // Setup the pool
        let (pool_pda, pool_bump, lp_token_mint_pda, lp_token_mint_bump) =
            test_instructions::add_pool(
                &program_test_ctx,
                &multisig_members_keypairs[0],
                payer_keypair,
                pool_name,
                &multisig_signers,
            )
            .await
            .unwrap();

        // Setup the custodies
        // Do it without ratio bound so we can provide liquidity without ratio limit error
        let custodies_info: Vec<SetupCustodyInfo> = {
            let mut custodies_info: Vec<SetupCustodyInfo> = Vec::new();

            let mut ratios = vec![];

            for (idx, custody_param) in custodies_params.iter().enumerate() {
                let mint_info = mints
                    .get(&custody_param.setup_custody_params.mint_name.to_string())
                    .unwrap();

                let custom_oracle_pda =
                    utils::get_custom_oracle_account(&pool_pda, &mint_info.pubkey).0;

                let target_ratio = 10_000 / (idx + 1) as u64;

                // Force ratio 0 to 100% to be able to provide liquidity
                ratios.push(TokenRatios {
                    target: target_ratio,
                    min: 0,
                    max: 10_000,
                });

                ratios.iter_mut().for_each(|x| x.target = target_ratio);

                if 10000 % (idx + 1) != 0 {
                    let len = ratios.len();
                    ratios[len - 1].target += 10_000 % (idx + 1) as u64;
                }

                let custody_pda = {
                    let add_custody_params = AddCustodyParams {
                        is_stable: custody_param.setup_custody_params.is_stable,
                        is_virtual: custody_param.setup_custody_params.is_virtual,
                        oracle: fixtures::oracle_params_regular(custom_oracle_pda),
                        pricing: custody_param
                            .setup_custody_params
                            .pricing_params
                            .unwrap_or_else(|| fixtures::pricing_params_regular(false)),
                        permissions: custody_param
                            .setup_custody_params
                            .permissions
                            .unwrap_or_else(fixtures::permissions_full),
                        fees: custody_param
                            .setup_custody_params
                            .fees
                            .unwrap_or_else(fixtures::fees_linear_regular),
                        borrow_rate: custody_param
                            .setup_custody_params
                            .borrow_rate
                            .unwrap_or_else(fixtures::borrow_rate_regular),

                        // in BPS, 10_000 = 100%
                        ratios: ratios.clone(),
                    };

                    test_instructions::add_custody(
                        &program_test_ctx,
                        &multisig_members_keypairs[0],
                        payer_keypair,
                        &pool_pda,
                        &mint_info.pubkey,
                        mint_info.decimals,
                        add_custody_params,
                        &multisig_signers,
                    )
                    .await
                    .unwrap()
                    .0
                };

                let publish_time = utils::get_current_unix_timestamp(&program_test_ctx).await;

                test_instructions::set_custom_oracle_price(
                    &program_test_ctx,
                    &multisig_members_keypairs[0],
                    payer_keypair,
                    &pool_pda,
                    &custody_pda,
                    &custom_oracle_pda,
                    SetCustomOraclePriceParams {
                        price: custody_param.setup_custody_params.initial_price,
                        expo: -(mint_info.decimals as i32),
                        conf: custody_param.setup_custody_params.initial_conf,
                        ema: custody_param.setup_custody_params.initial_price,
                        publish_time,
                    },
                    &multisig_signers,
                )
                .await
                .unwrap();

                custodies_info.push(SetupCustodyInfo {
                    custom_oracle_pda,
                    custody_pda,
                });
            }

            custodies_info
        };

        // Initialize LP staking
        {
            test_instructions::init_staking(
                &program_test_ctx,
                &multisig_members_keypairs[0],
                payer_keypair,
                staking_reward_token_mint,
                &lp_token_mint_pda,
                &InitStakingParams {
                    staking_type: StakingType::LP,
                },
                &multisig_signers,
            )
            .await
            .unwrap();
        }

        utils::warp_forward(&program_test_ctx, 1).await;

        // Initialize users token accounts for lp token mint
        {
            let users_pubkeys: Vec<Pubkey> = users
                .values()
                .into_iter()
                .map(|keypair| keypair.pubkey())
                .collect();

            utils::initialize_users_token_accounts(
                &program_test_ctx,
                vec![lp_token_mint_pda],
                users_pubkeys,
            )
            .await;
        }

        // Add liquidity
        for custody_param in custodies_params.as_slice() {
            let mint_info = mints
                .get(&custody_param.setup_custody_params.mint_name.to_string())
                .unwrap();

            let liquidity_provider = users
                .get(&custody_param.payer_user_name.to_string())
                .unwrap();

            println!(
                "adding liquidity for mint {}",
                custody_param.setup_custody_params.mint_name
            );

            if custody_param.liquidity_amount > 0 {
                test_instructions::add_liquidity(
                    &program_test_ctx,
                    liquidity_provider,
                    payer_keypair,
                    &pool_pda,
                    &mint_info.pubkey,
                    AddLiquidityParams {
                        amount_in: custody_param.liquidity_amount,
                        min_lp_amount_out: 1,
                    },
                )
                .await
                .unwrap();
            }
        }

        // Set proper ratios for custodies
        {
            let target_ratio = 10_000 / custodies_params.len() as u64;

            let mut ratios: Vec<TokenRatios> = custodies_params
                .iter()
                .map(|x| TokenRatios {
                    target: target_ratio,
                    min: x.setup_custody_params.min_ratio,
                    max: x.setup_custody_params.max_ratio,
                })
                .collect();

            if 10_000 % custodies_params.len() != 0 {
                let len = ratios.len();

                ratios[len - 1].target += 10_000 % custodies_params.len() as u64;
            }

            for (idx, _params) in custodies_params.as_slice().iter().enumerate() {
                utils::set_custody_ratios(
                    &program_test_ctx,
                    &multisig_members_keypairs[0],
                    payer_keypair,
                    &custodies_info[idx].custody_pda,
                    ratios.clone(),
                    &multisig_signers,
                )
                .await;
            }
        }

        TestSetup {
            program_test_ctx,
            root_authority_keypair: utils::copy_keypair(root_authority_keypair),
            payer_keypair: utils::copy_keypair(payer_keypair),
            users,
            mints,
            multisig_members,
            governance_realm_pda,
            gov_token_mint_pda,
            cortex_stake_reward_mint_name: cortex_stake_reward_mint_name.to_string(),
            lm_token_mint,
            pool_pda,
            pool_bump,
            lp_token_mint_pda,
            lp_token_mint_bump,
            custodies_info,
            clockwork_signatory: utils::copy_keypair(clockwork_signatory),
            clockwork_mint_reward_name: clockwork_mint_reward_name.to_string(),
        }
    }
}
