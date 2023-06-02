use {
    super::SetupCustodyInfo,
    crate::{
        instructions,
        utils::{self, fixtures},
    },
    bonfida_test_utils::ProgramTestExt,
    perpetuals::{
        instructions::AddLiquidityParams,
        state::{
            custody::{BorrowRateParams, Fees, PricingParams},
            perpetuals::Permissions,
            pool::TokenRatios,
        },
    },
    solana_program::pubkey::Pubkey,
    solana_program_test::{ProgramTest, ProgramTestContext},
    solana_sdk::{signature::Keypair, signer::Signer},
    std::{cell::RefCell, collections::HashMap},
};

pub struct NamedSetupCustodyWithLiquidityParams<'a> {
    pub setup_custody_params: NamedSetupCustodyParams<'a>,
    pub liquidity_amount: u64,

    // Who's adding the liquidity?
    pub payer_user_name: &'a str,
}

pub struct NamedSetupCustodyParams<'a> {
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
    pub program_test_ctx: RefCell<ProgramTestContext>,

    pub root_authority_keypair: Keypair,
    pub payer_keypair: Keypair,

    pub users: HashMap<String, Keypair>,
    pub mints: HashMap<String, MintInfo>,
    pub multisig_members: HashMap<String, Keypair>,

    pub pool_pda: Pubkey,
    pub pool_bump: u8,
    pub lp_token_mint_pda: Pubkey,
    pub lp_token_mint_bump: u8,
    pub custodies_info: Vec<SetupCustodyInfo>,
}

impl TestSetup {
    pub fn get_user_keypair_by_name(&self, name: &str) -> &Keypair {
        &self.users.get(&name.to_string()).unwrap()
    }

    pub fn get_multisig_member_keypair_by_name(&self, name: &str) -> &Keypair {
        &self.multisig_members.get(&name.to_string()).unwrap()
    }

    pub fn get_multisig_signers(&self) -> Vec<&Keypair> {
        self.multisig_members.values().collect()
    }

    pub fn get_mint_by_name(&self, name: &str) -> Pubkey {
        self.mints.get(&name.to_string()).unwrap().pubkey
    }

    pub async fn new(
        users_param: Vec<UserParam<'_>>,
        mints_param: Vec<MintParam<'_>>,
        multisig_members_names: Vec<&str>,
        pool_name: &str,
        custodies_named_params: Vec<NamedSetupCustodyWithLiquidityParams<'_>>,
    ) -> TestSetup {
        let mut program_test = ProgramTest::default();

        // Initialize keypairs
        let keypairs: Vec<Keypair> = utils::create_and_fund_multiple_accounts(
            &mut program_test,
            // 1 keypair per user
            users_param.len() +
            // payer
            1 +
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
            root_authority_keypair,
            program_authority_keypair,
            multisig_members_keypairs,
        ) = {
            (
                &keypairs[0..users_param.len()],
                keypairs.get(users_param.len()).unwrap(),
                keypairs.get(users_param.len() + 1).unwrap(),
                keypairs.get(users_param.len() + 2).unwrap(),
                &keypairs
                    [users_param.len() + 3..(users_param.len() + 3 + multisig_members_names.len())],
            )
        };

        let users = {
            let mut users: HashMap<String, Keypair> = HashMap::new();
            let mut i = 0;
            for user_param in users_param.as_slice() {
                users.insert(
                    user_param.name.to_string(),
                    utils::copy_keypair(&users_keypairs[i]),
                );
                i += 1;
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

        // Deploy program
        utils::add_perpetuals_program(&mut program_test, program_authority_keypair).await;

        // Start the client and connect to localnet validator
        let program_test_ctx: RefCell<ProgramTestContext> =
            RefCell::new(program_test.start_with_context().await);

        // Initialize multisig
        let multisig_members = {
            let mut multisig_members: HashMap<String, Keypair> = HashMap::new();
            let mut i: usize = 0;
            for multisig_member_name in multisig_members_names {
                multisig_members.insert(
                    multisig_member_name.to_string(),
                    utils::copy_keypair(&multisig_members_keypairs[i]),
                );
                i += 1;
            }

            multisig_members
        };

        let multisig_signers: Vec<&Keypair> = multisig_members.values().collect();

        instructions::test_init(
            &mut program_test_ctx.borrow_mut(),
            program_authority_keypair,
            fixtures::init_params_permissions_full(1),
            &multisig_signers,
        )
        .await
        .unwrap();

        // Initialize users token accounts for each mints
        {
            let mints_infos: Vec<&MintInfo> = mints.values().collect();
            let mints_pubkeys: Vec<Pubkey> =
                mints_infos.into_iter().map(|info| info.pubkey).collect();

            let users_keypairs: Vec<&Keypair> = users.values().collect();
            let users_pubkeys: Vec<Pubkey> = users_keypairs
                .into_iter()
                .map(|keypair| keypair.pubkey())
                .collect();

            utils::initialize_users_token_accounts(
                &mut program_test_ctx.borrow_mut(),
                mints_pubkeys,
                users_pubkeys,
            )
            .await;
        }

        utils::warp_forward(&mut program_test_ctx.borrow_mut(), 1).await;

        // Mint tokens for users to match specified balances
        {
            for user_param in users_param.as_slice() {
                for (mint_name, amount) in &user_param.token_balances {
                    let mint = mints.get(&mint_name.to_string()).unwrap().pubkey;
                    let user = users.get(&user_param.name.to_string()).unwrap().pubkey();

                    let (ata, _) = utils::find_associated_token_account(&user, &mint);

                    utils::mint_tokens(
                        &mut program_test_ctx.borrow_mut(),
                        root_authority_keypair,
                        &mint,
                        &ata,
                        *amount,
                    )
                    .await;
                }
            }
        }

        let custodies_params: Vec<utils::SetupCustodyWithLiquidityParams> = {
            // Build custodies params using informations given to this function
            // Basically this function received mint_name and have to replace it with actual mint

            custodies_named_params
                .into_iter()
                .map(|params| {
                    let payer = users.get(&params.payer_user_name.to_string()).unwrap();
                    let mint_info = mints
                        .get(&params.setup_custody_params.mint_name.to_string())
                        .unwrap();

                    utils::SetupCustodyWithLiquidityParams {
                        setup_custody_params: utils::SetupCustodyParams {
                            mint: mint_info.pubkey,
                            decimals: mint_info.decimals,
                            is_virtual: params.setup_custody_params.is_virtual,
                            is_stable: params.setup_custody_params.is_stable,
                            target_ratio: params.setup_custody_params.target_ratio,
                            min_ratio: params.setup_custody_params.min_ratio,
                            max_ratio: params.setup_custody_params.max_ratio,
                            initial_price: params.setup_custody_params.initial_price,
                            initial_conf: params.setup_custody_params.initial_conf,
                            pricing_params: params.setup_custody_params.pricing_params,
                            permissions: params.setup_custody_params.permissions,
                            fees: params.setup_custody_params.fees,
                            borrow_rate: params.setup_custody_params.borrow_rate,
                        },
                        liquidity_amount: params.liquidity_amount,
                        payer: utils::copy_keypair(payer),
                    }
                })
                .collect()
        };

        // Setup the pool without ratio bound so we can provide liquidity without ratio limit error
        let (pool_pda, pool_bump, lp_token_mint_pda, lp_token_mint_bump, custodies_info) =
            utils::setup_pool_with_custodies(
                &mut program_test_ctx.borrow_mut(),
                &multisig_members_keypairs[0],
                pool_name,
                payer_keypair,
                &multisig_signers,
                custodies_params
                    .iter()
                    .map(|e| {
                        let mut params = e.setup_custody_params;

                        params.max_ratio = 10_000;
                        params.min_ratio = 0;

                        params
                    })
                    .collect(),
            )
            .await;

        // Initialize users token accounts for lp token mint
        {
            let users_keypairs: Vec<&Keypair> = users.values().collect();
            let users_pubkeys: Vec<Pubkey> = users_keypairs
                .into_iter()
                .map(|keypair| keypair.pubkey())
                .collect();

            utils::initialize_users_token_accounts(
                &mut program_test_ctx.borrow_mut(),
                vec![lp_token_mint_pda],
                users_pubkeys,
            )
            .await;
        }

        // Add liquidity
        for params in custodies_params.as_slice() {
            println!(
                "adding liquidity for mint {}",
                params.setup_custody_params.mint
            );

            if params.liquidity_amount > 0 {
                instructions::test_add_liquidity(
                    &mut program_test_ctx.borrow_mut(),
                    &params.payer,
                    payer_keypair,
                    &pool_pda,
                    &params.setup_custody_params.mint,
                    AddLiquidityParams {
                        amount_in: params.liquidity_amount,
                        min_lp_amount_out: 1,
                    },
                )
                .await
                .unwrap();
            }
        }

        // Set proper ratios
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
                &mut program_test_ctx.borrow_mut(),
                &multisig_members_keypairs[0],
                payer_keypair,
                &custodies_info[idx].custody_pda,
                ratios.clone(),
                &multisig_signers,
            )
            .await;
        }

        TestSetup {
            program_test_ctx,
            root_authority_keypair: utils::copy_keypair(root_authority_keypair),
            payer_keypair: utils::copy_keypair(payer_keypair),
            users,
            mints,
            multisig_members,
            pool_pda,
            pool_bump,
            lp_token_mint_pda,
            lp_token_mint_bump,
            custodies_info,
        }
    }
}
