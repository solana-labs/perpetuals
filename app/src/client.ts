import {
  setProvider,
  Program,
  AnchorProvider,
  workspace,
  utils,
  BN,
} from "@coral-xyz/anchor";
import { Perpetuals } from "../../target/types/perpetuals";
import {
  PublicKey,
  SystemProgram,
  Keypair,
  SYSVAR_RENT_PUBKEY,
  AccountMeta,
  ComputeBudgetProgram,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddress,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { sha256 } from "@noble/hashes/sha256";
import { encode } from "bs58";
import { readFileSync } from "fs";
import {
  TokenRatio,
  PositionSide,
  InitParams,
  OracleParams,
  PricingParams,
  Permissions,
  Fees,
  BorrowRateParams,
  SetCustomOraclePriceParams,
  AmountAndFee,
  NewPositionPricesAndFee,
  PriceAndFee,
  ProfitAndLoss,
  SwapAmountAndFees,
  Custody,
} from "./types";
import {
  GoverningTokenConfigAccountArgs,
  GoverningTokenType,
  MintMaxVoteWeightSource,
  MintMaxVoteWeightSourceType,
  getGovernanceProgramVersion,
  getGoverningTokenHoldingAddress,
  getRealmConfigAddress,
  getTokenOwnerRecordAddress,
  withCreateGovernance,
  withCreateRealm,
} from "@solana/spl-governance";

const governanceProgram = new PublicKey(
  "GovER5Lthms3bLBqWub97yVrMmEogzX7xNjdXpPPCVZw"
);

export class PerpetualsClient {
  provider: AnchorProvider;
  program: Program<Perpetuals>;
  admin: Keypair;

  // pdas
  multisig: { publicKey: PublicKey; bump: number };
  authority: { publicKey: PublicKey; bump: number };
  perpetuals: { publicKey: PublicKey; bump: number };
  lmTokenMint: { publicKey: PublicKey; bump: number };
  lmStaking: { publicKey: PublicKey; bump: number };
  cortex: { publicKey: PublicKey; bump: number };
  governanceTokenMint: { publicKey: PublicKey; bump: number };
  lmStakingStakedTokenVault: { publicKey: PublicKey; bump: number };
  lmStakingRewardTokenVault: { publicKey: PublicKey; bump: number };
  lmStakingLmRewardTokenVault: { publicKey: PublicKey; bump: number };

  constructor(clusterUrl: string, adminKey: string) {
    this.provider = AnchorProvider.local(clusterUrl, {
      commitment: "confirmed",
      preflightCommitment: "confirmed",
    });

    setProvider(this.provider);

    this.program = workspace.Perpetuals as Program<Perpetuals>;

    this.admin = Keypair.fromSecretKey(
      new Uint8Array(JSON.parse(readFileSync(adminKey).toString()))
    );

    this.multisig = this.findProgramAddress("multisig");
    this.authority = this.findProgramAddress("transfer_authority");
    this.perpetuals = this.findProgramAddress("perpetuals");
    this.lmTokenMint = this.findProgramAddress("lm_token_mint");
    this.lmStaking = this.findProgramAddress("staking", [
      this.lmTokenMint.publicKey,
    ]);
    this.cortex = this.findProgramAddress("cortex");
    this.governanceTokenMint = this.findProgramAddress("governance_token_mint");
    this.lmStakingStakedTokenVault = this.findProgramAddress(
      "staking_staked_token_vault",
      [this.lmStaking.publicKey]
    );
    this.lmStakingRewardTokenVault = this.findProgramAddress(
      "staking_reward_token_vault",
      [this.lmStaking.publicKey]
    );
    this.lmStakingLmRewardTokenVault = this.findProgramAddress(
      "staking_lm_reward_token_vault",
      [this.lmStaking.publicKey]
    );

    BN.prototype.toJSON = function () {
      return this.toString(10);
    };
  }

  getVestPda = (
    owner: PublicKey
  ): {
    publicKey: PublicKey;
    bump: number;
  } => this.findProgramAddress("vest", [owner]);

  findProgramAddress = (
    label: string,
    extraSeeds = null
  ): {
    publicKey: PublicKey;
    bump: number;
  } => {
    const seeds = [Buffer.from(utils.bytes.utf8.encode(label))];

    if (extraSeeds) {
      for (const extraSeed of extraSeeds) {
        if (typeof extraSeed === "string") {
          seeds.push(Buffer.from(utils.bytes.utf8.encode(extraSeed)));
        } else if (Array.isArray(extraSeed)) {
          seeds.push(Buffer.from(extraSeed));
        } else {
          seeds.push(extraSeed.toBuffer());
        }
      }
    }

    const [publicKey, bump] = PublicKey.findProgramAddressSync(
      seeds,
      this.program.programId
    );

    return { publicKey, bump };
  };

  adjustTokenRatios = (ratios: TokenRatio[]): TokenRatio[] => {
    if (ratios.length == 0) {
      return ratios;
    }

    const target = Math.floor(10_000 / ratios.length);

    for (const ratio of ratios) {
      ratio.target = new BN(target);
    }

    if (10_000 % ratios.length !== 0) {
      ratios[ratios.length - 1].target = new BN(
        target + (10_000 % ratios.length)
      );
    }

    return ratios;
  };

  getPerpetuals = async () => {
    return this.program.account.perpetuals.fetch(this.perpetuals.publicKey);
  };

  getPoolKey = (name: string): PublicKey => {
    return this.findProgramAddress("pool", [name]).publicKey;
  };

  getPool = async (name: string) => {
    console.log(`Pool key: ${this.getPoolKey(name).toBase58()}`);

    return this.program.account.pool.fetch(this.getPoolKey(name));
  };

  getPools = async () => {
    //return this.program.account.pool.all();
    const perpetuals = await this.getPerpetuals();
    return this.program.account.pool.fetchMultiple(perpetuals.pools);
  };

  getPoolLpTokenKey = (name: string): PublicKey => {
    return this.findProgramAddress("lp_token_mint", [this.getPoolKey(name)])
      .publicKey;
  };

  getCustodyKey = (poolName: string, tokenMint: PublicKey): PublicKey => {
    return this.findProgramAddress("custody", [
      this.getPoolKey(poolName),
      tokenMint,
    ]).publicKey;
  };

  getDaoRealmKey = (realmName: string): PublicKey =>
    PublicKey.findProgramAddressSync(
      [Buffer.from("governance"), Buffer.from(realmName)],
      governanceProgram
    )[0];

  getGovernanceTokenKey = (): PublicKey => {
    return this.findProgramAddress("governance_token_mint").publicKey;
  };

  getCustodyTokenAccountKey = (
    poolName: string,
    tokenMint: PublicKey
  ): PublicKey => {
    return this.findProgramAddress("custody_token_account", [
      this.getPoolKey(poolName),
      tokenMint,
    ]).publicKey;
  };

  getCustodyOracleAccountKey = async (
    poolName: string,
    tokenMint: PublicKey
  ): Promise<PublicKey> => {
    return (await this.getCustody(poolName, tokenMint)).oracle.oracleAccount;
  };

  getCustodyCustomOracleAccountKey = (
    poolName: string,
    tokenMint: PublicKey
  ): PublicKey => {
    return this.findProgramAddress("oracle_account", [
      this.getPoolKey(poolName),
      tokenMint,
    ]).publicKey;
  };

  getCustody = async (poolName: string, tokenMint: PublicKey) => {
    return this.program.account.custody.fetch(
      this.getCustodyKey(poolName, tokenMint)
    );
  };

  getCustodies = async (poolName: string): Promise<Custody[]> => {
    //return this.program.account.custody.all();
    const pool = await this.getPool(poolName);
    const custodies = (await this.program.account.custody.fetchMultiple(
      pool.custodies
    )) as (Custody | null)[];

    if (custodies.some((custody) => !custody)) {
      throw new Error("Error loading custodies");
    }

    return custodies;
  };

  getCustodyMetas = async (poolName: string): Promise<AccountMeta[]> => {
    const pool = await this.getPool(poolName);
    const custodies = (await this.program.account.custody.fetchMultiple(
      pool.custodies
    )) as (Custody | null)[];

    if (custodies.some((custody) => !custody)) {
      throw new Error("Error loading custodies");
    }

    const custodyMetas: AccountMeta[] = [];

    for (const custody of pool.custodies) {
      custodyMetas.push({
        isSigner: false,
        isWritable: false,
        pubkey: custody,
      });
    }

    for (const custody of custodies) {
      custodyMetas.push({
        isSigner: false,
        isWritable: false,
        pubkey: custody.oracle.oracleAccount,
      });
    }

    return custodyMetas;
  };

  getCollateralCustodyMint = async (
    wallet: PublicKey,
    poolName: string,
    tokenMint: PublicKey,
    side: PositionSide
  ): Promise<PublicKey> => {
    const custodyAccount = (
      await this.getUserPosition(wallet, poolName, tokenMint, side)
    ).collateralCustody;

    return (await this.program.account.custody.fetch(custodyAccount)).mint;
  };

  getMultisig = async () => {
    return this.program.account.multisig.fetch(this.multisig.publicKey);
  };

  getPositionKey = (
    wallet: PublicKey,
    poolName: string,
    tokenMint: PublicKey,
    side: PositionSide
  ): PublicKey => {
    const pool = this.getPoolKey(poolName);
    const custody = this.getCustodyKey(poolName, tokenMint);

    return this.findProgramAddress("position", [
      wallet,
      pool,
      custody,
      side === "long" ? [1] : [0],
    ]).publicKey;
  };

  getUserPosition = async (
    wallet: PublicKey,
    poolName: string,
    tokenMint: PublicKey,
    side: PositionSide
  ) => {
    return this.program.account.position.fetch(
      this.getPositionKey(wallet, poolName, tokenMint, side)
    );
  };

  getUserPositions = async (wallet: PublicKey) => {
    const data = encode(
      Buffer.concat([
        this.getAccountDiscriminator("Position"),
        wallet.toBuffer(),
      ])
    );

    const positions = await this.provider.connection.getProgramAccounts(
      this.program.programId,
      {
        filters: [{ dataSize: 232 }, { memcmp: { bytes: data, offset: 0 } }],
      }
    );

    return Promise.all(
      positions.map((position) => {
        return this.program.account.position.fetch(position.pubkey);
      })
    );
  };

  getPoolTokenPositions = async (poolName: string, tokenMint: PublicKey) => {
    const poolKey = this.getPoolKey(poolName);
    const custodyKey = this.getCustodyKey(poolName, tokenMint);
    const data = encode(
      Buffer.concat([poolKey.toBuffer(), custodyKey.toBuffer()])
    );
    const positions = await this.provider.connection.getProgramAccounts(
      this.program.programId,
      {
        filters: [{ dataSize: 232 }, { memcmp: { bytes: data, offset: 40 } }],
      }
    );

    return Promise.all(
      positions.map((position) => {
        return this.program.account.position.fetch(position.pubkey);
      })
    );
  };

  getAllPositions = async () => {
    return this.program.account.position.all();
  };

  getAccountDiscriminator = (name: string): Buffer => {
    return Buffer.from(sha256(`account:${name}`).slice(0, 8));
  };

  getMethodDiscriminator = (name: string): Buffer => {
    return Buffer.from(sha256(`global:${name}`).slice(0, 8));
  };

  getTime(): number {
    const now = new Date();
    const utcMilllisecondsSinceEpoch =
      now.getTime() + now.getTimezoneOffset() * 60 * 1_000;

    return utcMilllisecondsSinceEpoch / 1_000;
  }

  log = (...messages: string[]): void => {
    const date = new Date();
    const dateStr = date.toDateString();
    const time = date.toLocaleTimeString();

    console.log(`[${dateStr} ${time}] ${messages.join(", ")}`);
  };

  prettyPrint = (v: any): void => {
    console.log(JSON.stringify(v, null, 2));
  };

  ///////
  // instructions

  createDaoGovernance = async (realmName: string): Promise<string> => {
    const realmPubkey = this.getDaoRealmKey(realmName);

    const instructions: TransactionInstruction[] = [];

    const programVersion = await getGovernanceProgramVersion(
      this.provider.connection,
      governanceProgram
    );

    // TODO
    // withCreateGovernance(instructions, governanceProgram, programVersion);
    return "";
  };

  createDaoRealm = async (
    name: string,
    minCommunityWeightToCreateGovernance: BN
  ): Promise<PublicKey> => {
    // Use the Admin as the authority
    const realmAuthority = this.provider.wallet.publicKey;
    const payer = realmAuthority;

    const instructions: TransactionInstruction[] = [];

    const programVersion = await getGovernanceProgramVersion(
      this.provider.connection,
      governanceProgram
    );

    const communityMint = this.getGovernanceTokenKey();

    const communityMintMaxVoteWeightSource = new MintMaxVoteWeightSource({
      /// Fraction (10^10 precision) of the governing mint supply is used as max vote weight
      /// The default is 100% (10^10) to use all available mint supply for voting
      type: MintMaxVoteWeightSourceType.SupplyFraction,

      // 100%
      value: new BN(100),
    });

    const communityTokenConfig: GoverningTokenConfigAccountArgs =
      new GoverningTokenConfigAccountArgs({
        tokenType: GoverningTokenType.Membership,
        voterWeightAddin: undefined,
        maxVoterWeightAddin: undefined,
      });

    const realmPubkey = await withCreateRealm(
      instructions,
      governanceProgram,
      programVersion,
      name,
      realmAuthority,
      communityMint,
      payer,
      undefined /* council mint */,
      communityMintMaxVoteWeightSource,
      // Governance token mint is 6 decimals
      new BN(10 ** 6).mul(minCommunityWeightToCreateGovernance),
      communityTokenConfig,
      undefined /* councilTokenConfig */
    );

    const tx = new Transaction();

    tx.add(...instructions);
    tx.recentBlockhash = (
      await this.provider.connection.getLatestBlockhash()
    ).blockhash;
    tx.feePayer = payer;

    const signedTransaction = await this.provider.wallet.signTransaction(tx);

    const txId = await this.provider.connection.sendRawTransaction(
      signedTransaction.serialize()
    );

    const confirmationStatus =
      await this.provider.connection.confirmTransaction(txId, "confirmed");

    if (confirmationStatus.value.err) {
      console.error(`Transaction failed: ${confirmationStatus.value.err}`);
    } else {
      console.log(`Transaction succeeded: ${txId}`);
    }

    return realmPubkey;
  };

  claimVest = async (): Promise<string> => {
    const lmTokenAccount = await getAssociatedTokenAddress(
      this.lmTokenMint.publicKey,
      this.provider.wallet.publicKey
    );

    const cortexAccount = await this.program.account.cortex.fetch(
      this.cortex.publicKey
    );

    const preInstructions: TransactionInstruction[] = [];

    /*const modifyComputeUnits = ComputeBudgetProgram.setComputeUnitLimit({
      units: 500_000,
    });

    preInstructions.push(modifyComputeUnits);*/

    // Create LM ATA if doesn't exist
    if (!(await this.provider.connection.getAccountInfo(lmTokenAccount))) {
      preInstructions.push(
        createAssociatedTokenAccountInstruction(
          this.provider.wallet.publicKey,
          lmTokenAccount,
          this.provider.wallet.publicKey,
          this.lmTokenMint.publicKey
        )
      );
    }

    return this.program.methods
      .claimVest()
      .accounts({
        owner: this.provider.wallet.publicKey,
        receivingAccount: lmTokenAccount,
        transferAuthority: this.authority.publicKey,
        cortex: this.cortex.publicKey,
        perpetuals: this.perpetuals.publicKey,
        vest: this.getVestPda(this.provider.wallet.publicKey).publicKey,
        lmTokenMint: this.lmTokenMint.publicKey,
        governanceTokenMint: this.governanceTokenMint.publicKey,
        governanceRealm: cortexAccount.governanceRealm,
        governanceRealmConfig: await getRealmConfigAddress(
          governanceProgram,
          cortexAccount.governanceRealm
        ),
        governanceGoverningTokenHolding: await getGoverningTokenHoldingAddress(
          governanceProgram,
          cortexAccount.governanceRealm,
          this.governanceTokenMint.publicKey
        ),
        governanceGoverningTokenOwnerRecord: await getTokenOwnerRecordAddress(
          governanceProgram,
          cortexAccount.governanceRealm,
          this.governanceTokenMint.publicKey,
          this.provider.wallet.publicKey
        ),
        governanceProgram,
        perpetualsProgram: this.program.programId,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .preInstructions(preInstructions)
      .rpc()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  init = async (
    admins: PublicKey[],
    lmStakingRewardTokenMint: PublicKey,
    governanceRealm: PublicKey,
    config: InitParams
  ): Promise<void> => {
    const perpetualsProgramData = PublicKey.findProgramAddressSync(
      [this.program.programId.toBuffer()],
      new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111")
    )[0];

    const adminMetas = [];

    for (const admin of admins) {
      adminMetas.push({
        isSigner: false,
        isWritable: false,
        pubkey: admin,
      });
    }

    const modifyComputeUnits = ComputeBudgetProgram.setComputeUnitLimit({
      units: 500_000,
    });

    await this.program.methods
      .init(config)
      .accounts({
        upgradeAuthority: this.provider.wallet.publicKey,
        multisig: this.multisig.publicKey,
        transferAuthority: this.authority.publicKey,
        lmStaking: this.lmStaking.publicKey,
        cortex: this.cortex.publicKey,
        lmTokenMint: this.lmTokenMint.publicKey,
        governanceTokenMint: this.governanceTokenMint.publicKey,
        lmStakingStakedTokenVault: this.lmStakingStakedTokenVault.publicKey,
        lmStakingRewardTokenVault: this.lmStakingRewardTokenVault.publicKey,
        lmStakingLmRewardTokenVault: this.lmStakingLmRewardTokenVault.publicKey,
        lmStakingRewardTokenMint: lmStakingRewardTokenMint,
        perpetuals: this.perpetuals.publicKey,
        perpetualsProgram: this.program.programId,
        perpetualsProgramData,
        governanceRealm,
        governanceProgram,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .preInstructions([modifyComputeUnits])
      .remainingAccounts(adminMetas)
      .rpc()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  // Using normal way of building the Transaction triggers a borsh error
  // due to the enum not being handled properly
  // reverted to a manual build of the ix with manual data encoding
  initLpStaking = async (
    poolName: string,
    stakingRewardTokenMint: PublicKey
  ): Promise<void> => {
    const lpTokenMint = this.getPoolLpTokenKey(poolName);

    const { publicKey: lpStaking } = this.findProgramAddress("staking", [
      lpTokenMint,
    ]);

    const { publicKey: stakingStakedTokenVault } = this.findProgramAddress(
      "staking_staked_token_vault",
      [lpStaking]
    );
    const { publicKey: stakingRewardTokenVault } = this.findProgramAddress(
      "staking_reward_token_vault",
      [lpStaking]
    );
    const { publicKey: stakingLmRewardTokenVault } = this.findProgramAddress(
      "staking_lm_reward_token_vault",
      [lpStaking]
    );

    const dataBuff = Buffer.from([
      ...this.getMethodDiscriminator("init_staking"),
      // LP or LM enum
      1, // 1 = LP
    ]);

    const ix = new TransactionInstruction({
      data: dataBuff,
      keys: [
        {
          // admin
          isSigner: true,
          isWritable: false,
          pubkey: this.admin.publicKey,
        },
        {
          // payer
          isSigner: true,
          isWritable: true,
          pubkey: this.provider.wallet.publicKey,
        },
        {
          // multisig
          isSigner: false,
          isWritable: true,
          pubkey: this.multisig.publicKey,
        },
        {
          // transferAuthority
          isSigner: false,
          isWritable: false,
          pubkey: this.authority.publicKey,
        },
        {
          // staking
          isSigner: false,
          isWritable: true,
          pubkey: lpStaking,
        },
        {
          // lmTokenMint
          isSigner: false,
          isWritable: true,
          pubkey: this.lmTokenMint.publicKey,
        },
        {
          // cortex
          isSigner: false,
          isWritable: true,
          pubkey: this.cortex.publicKey,
        },
        {
          // perpetuals
          isSigner: false,
          isWritable: true,
          pubkey: this.perpetuals.publicKey,
        },
        {
          // stakingStakedTokenVault
          isSigner: false,
          isWritable: true,
          pubkey: stakingStakedTokenVault,
        },
        {
          // stakingRewardTokenVault
          isSigner: false,
          isWritable: true,
          pubkey: stakingRewardTokenVault,
        },
        {
          // stakingLmRewardTokenVault
          isSigner: false,
          isWritable: true,
          pubkey: stakingLmRewardTokenVault,
        },
        {
          // stakingRewardTokenMint
          isSigner: false,
          isWritable: false,
          pubkey: stakingRewardTokenMint,
        },
        {
          // stakingStakedTokenMint
          isSigner: false,
          isWritable: false,
          pubkey: lpTokenMint,
        },
        {
          // systemProgram
          isSigner: false,
          isWritable: false,
          pubkey: SystemProgram.programId,
        },
        {
          // tokenProgram
          isSigner: false,
          isWritable: false,
          pubkey: TOKEN_PROGRAM_ID,
        },
        {
          // rent
          isSigner: false,
          isWritable: false,
          pubkey: SYSVAR_RENT_PUBKEY,
        },
      ],
      programId: this.program.programId,
    });

    const tx = new Transaction();

    tx.add(ix);

    tx.feePayer = this.provider.wallet.publicKey;

    const { blockhash, lastValidBlockHeight } =
      await this.provider.connection.getLatestBlockhash();

    tx.recentBlockhash = blockhash;

    const signedTransaction = await this.provider.wallet.signTransaction(tx);
    const txId = await this.provider.connection.sendRawTransaction(
      signedTransaction.serialize()
    );

    console.log(`Transaction succeeded: ${txId}`);

    const resp = await this.provider.connection.confirmTransaction({
      blockhash,
      lastValidBlockHeight,
      signature: txId,
    });

    if (resp.value.err) {
      throw resp.value.err;
    }
  };

  setAdminSigners = async (
    admins: PublicKey[],
    minSignatures: number
  ): Promise<void> => {
    const adminMetas = [];

    for (const admin of admins) {
      adminMetas.push({
        isSigner: false,
        isWritable: false,
        pubkey: admin,
      });
    }

    try {
      await this.program.methods
        .setAdminSigners({
          minSignatures,
        })
        .accounts({
          admin: this.admin.publicKey,
          multisig: this.multisig.publicKey,
        })
        .remainingAccounts(adminMetas)
        .signers([this.admin])
        .rpc();
    } catch (err) {
      console.log(err);
      throw err;
    }
  };

  addPool = async (name: string): Promise<void> => {
    await this.program.methods
      .addPool({ name })
      .accounts({
        admin: this.admin.publicKey,
        multisig: this.multisig.publicKey,
        transferAuthority: this.authority.publicKey,
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(name),
        lpTokenMint: this.getPoolLpTokenKey(name),
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .signers([this.admin])
      .rpc()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  removePool = async (name: string): Promise<void> => {
    await this.program.methods
      .removePool({})
      .accounts({
        admin: this.admin.publicKey,
        multisig: this.multisig.publicKey,
        transferAuthority: this.authority.publicKey,
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(name),
        systemProgram: SystemProgram.programId,
      })
      .signers([this.admin])
      .rpc()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  addCustody = async (
    poolName: string,
    tokenMint: PublicKey,
    isStable: boolean,
    isVirtual: boolean,
    oracleConfig: OracleParams,
    pricingConfig: PricingParams,
    permissions: Permissions,
    fees: Fees,
    borrowRate: BorrowRateParams,
    ratios: TokenRatio[]
  ): Promise<void> => {
    await this.program.methods
      .addCustody({
        isStable,
        isVirtual,
        oracle: oracleConfig,
        pricing: pricingConfig,
        permissions,
        fees,
        borrowRate,
        ratios,
      })
      .accounts({
        admin: this.admin.publicKey,
        multisig: this.multisig.publicKey,
        transferAuthority: this.authority.publicKey,
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
        custody: this.getCustodyKey(poolName, tokenMint),
        custodyTokenAccount: this.getCustodyTokenAccountKey(
          poolName,
          tokenMint
        ),
        custodyTokenMint: tokenMint,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .signers([this.admin])
      .rpc()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  removeCustody = async (
    poolName: string,
    tokenMint: PublicKey,
    ratios: TokenRatio[]
  ): Promise<void> => {
    await this.program.methods
      .removeCustody({ ratios })
      .accounts({
        admin: this.admin.publicKey,
        multisig: this.multisig.publicKey,
        transferAuthority: this.authority.publicKey,
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
        custody: this.getCustodyKey(poolName, tokenMint),
        custodyTokenAccount: this.getCustodyTokenAccountKey(
          poolName,
          tokenMint
        ),
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([this.admin])
      .rpc()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  upgradeCustody = async (
    poolName: string,
    tokenMint: PublicKey
  ): Promise<void> => {
    await this.program.methods
      .upgradeCustody({})
      .accounts({
        admin: this.admin.publicKey,
        multisig: this.multisig.publicKey,
        pool: this.getPoolKey(poolName),
        custody: this.getCustodyKey(poolName, tokenMint),
        systemProgram: SystemProgram.programId,
      })
      .signers([this.admin])
      .rpc()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  setCustomOraclePrice = async (
    poolName: string,
    tokenMint: PublicKey,
    priceConfig: SetCustomOraclePriceParams
  ): Promise<void> => {
    await this.program.methods
      .setCustomOraclePrice(priceConfig)
      .accounts({
        admin: this.admin.publicKey,
        multisig: this.multisig.publicKey,
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
        custody: this.getCustodyKey(poolName, tokenMint),
        oracleAccount: this.getCustodyCustomOracleAccountKey(
          poolName,
          tokenMint
        ),
        systemProgram: SystemProgram.programId,
      })
      .signers([this.admin])
      .rpc()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  addLiquidity = async (
    poolName: string,
    tokenMint: PublicKey,
    amountIn: BN,
    minLpAmountOut: BN
  ): Promise<void> => {
    const lpTokenMint = this.getPoolLpTokenKey(poolName);

    const lpStaking = this.findProgramAddress("staking", [
      lpTokenMint,
    ]).publicKey;

    const lpStakingAccount = await this.program.account.staking.fetch(
      lpStaking
    );

    const stakingRewardTokenMint: PublicKey = lpStakingAccount.rewardTokenMint;

    const lpTokenAccount = await getAssociatedTokenAddress(
      lpTokenMint,
      this.provider.wallet.publicKey
    );

    const lmTokenAccount = await getAssociatedTokenAddress(
      this.lmTokenMint.publicKey,
      this.provider.wallet.publicKey
    );

    const modifyComputeUnits = ComputeBudgetProgram.setComputeUnitLimit({
      units: 500_000,
    });

    const preInstructions: TransactionInstruction[] = [modifyComputeUnits];

    // init LP token ATA if it doesn't exist
    if (!(await this.provider.connection.getAccountInfo(lpTokenAccount))) {
      preInstructions.push(
        createAssociatedTokenAccountInstruction(
          this.provider.wallet.publicKey,
          lpTokenAccount,
          this.provider.wallet.publicKey,
          lpTokenMint
        )
      );
    }

    // init LM token ATA if it doesn't exist
    if (!(await this.provider.connection.getAccountInfo(lmTokenAccount))) {
      preInstructions.push(
        createAssociatedTokenAccountInstruction(
          this.provider.wallet.publicKey,
          lmTokenAccount,
          this.provider.wallet.publicKey,
          this.lmTokenMint.publicKey
        )
      );
    }

    await this.program.methods
      .addLiquidity({ amountIn, minLpAmountOut })
      .accounts({
        owner: this.provider.wallet.publicKey,
        fundingAccount: await getAssociatedTokenAddress(
          tokenMint,
          this.provider.wallet.publicKey
        ),
        lpTokenAccount,
        lmTokenAccount,
        transferAuthority: this.authority.publicKey,
        lmStaking: this.lmStaking.publicKey,
        lpStaking,
        cortex: this.cortex.publicKey,
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
        stakingRewardTokenCustody: this.getCustodyKey(
          poolName,
          stakingRewardTokenMint
        ),
        stakingRewardTokenCustodyOracleAccount:
          await this.getCustodyOracleAccountKey(
            poolName,
            stakingRewardTokenMint
          ),
        stakingRewardTokenCustodyTokenAccount: this.getCustodyTokenAccountKey(
          poolName,
          stakingRewardTokenMint
        ),
        custody: this.getCustodyKey(poolName, tokenMint),
        custodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          tokenMint
        ),
        custodyTokenAccount: this.getCustodyTokenAccountKey(
          poolName,
          tokenMint
        ),
        lmStakingRewardTokenVault: this.lmStakingRewardTokenVault.publicKey,
        lpStakingRewardTokenVault: this.findProgramAddress(
          "staking_reward_token_vault",
          [lpStaking]
        ).publicKey,
        lmTokenMint: this.lmTokenMint.publicKey,
        lpTokenMint,
        stakingRewardTokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        perpetualsProgram: this.program.programId,
      })
      .remainingAccounts(await this.getCustodyMetas(poolName))
      .preInstructions(preInstructions)
      .rpc()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  liquidate = async (
    wallet: PublicKey,
    poolName: string,
    tokenMint: PublicKey,
    collateralMint: PublicKey,
    side: PositionSide,
    receivingAccount: PublicKey,
    rewardsReceivingAccount: PublicKey
  ): Promise<void> => {
    await this.program.methods
      .liquidate({})
      .accounts({
        signer: this.provider.wallet.publicKey,
        receivingAccount,
        rewardsReceivingAccount,
        transferAuthority: this.authority.publicKey,
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
        position: this.getPositionKey(wallet, poolName, tokenMint, side),
        custody: this.getCustodyKey(poolName, tokenMint),
        custodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          tokenMint
        ),
        collateralCustody: this.getCustodyKey(poolName, collateralMint),
        collateralCustodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          collateralMint
        ),
        collateralCustodyTokenAccount: this.getCustodyTokenAccountKey(
          poolName,
          collateralMint
        ),
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  openPosition = async (
    poolName: string,
    tokenMint: PublicKey,
    collateralMint: PublicKey,
    side: PositionSide,
    price: BN,
    collateral: BN,
    size: BN
  ): Promise<void> => {
    await this.program.methods
      .openPosition({
        price,
        collateral,
        size,
        side: side === "long" ? { long: {} } : { short: {} },
      })
      .accounts({
        owner: this.provider.wallet.publicKey,
        fundingAccount: await getAssociatedTokenAddress(
          collateralMint,
          this.provider.wallet.publicKey
        ),
        transferAuthority: this.authority.publicKey,
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
        position: this.getPositionKey(
          this.provider.wallet.publicKey,
          poolName,
          tokenMint,
          side
        ),
        custody: this.getCustodyKey(poolName, tokenMint),
        custodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          tokenMint
        ),
        collateralCustody: this.getCustodyKey(poolName, collateralMint),
        collateralCustodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          collateralMint
        ),
        collateralCustodyTokenAccount: this.getCustodyTokenAccountKey(
          poolName,
          collateralMint
        ),
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  addVest = async (
    beneficiaryWalet: PublicKey,
    amount: BN,
    unlockStartTimestamp: BN,
    unlockEndTimestamp: BN
  ): Promise<string> => {
    const cortexAccount = await this.program.account.cortex.fetch(
      this.cortex.publicKey
    );

    return this.program.methods
      .addVest({
        amount,
        unlockStartTimestamp,
        unlockEndTimestamp,
      })
      .accounts({
        admin: this.admin.publicKey,
        owner: beneficiaryWalet,
        payer: this.provider.wallet.publicKey,
        multisig: this.multisig.publicKey,
        transferAuthority: this.authority.publicKey,
        cortex: this.cortex.publicKey,
        perpetuals: this.perpetuals.publicKey,
        vest: this.getVestPda(beneficiaryWalet).publicKey,
        lmTokenMint: this.lmTokenMint.publicKey,
        governanceTokenMint: this.governanceTokenMint.publicKey,
        governanceRealm: cortexAccount.governanceRealm,
        governanceRealmConfig: await getRealmConfigAddress(
          governanceProgram,
          cortexAccount.governanceRealm
        ),
        governanceGoverningTokenHolding: await getGoverningTokenHoldingAddress(
          governanceProgram,
          cortexAccount.governanceRealm,
          this.governanceTokenMint.publicKey
        ),
        governanceGoverningTokenOwnerRecord: await getTokenOwnerRecordAddress(
          governanceProgram,
          cortexAccount.governanceRealm,
          this.governanceTokenMint.publicKey,
          beneficiaryWalet
        ),
        governanceProgram,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .rpc();
  };

  getOraclePrice = async (
    poolName: string,
    tokenMint: PublicKey,
    ema: boolean
  ): Promise<BN> => {
    return this.program.methods
      .getOraclePrice({
        ema,
      })
      .accounts({
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
        custody: this.getCustodyKey(poolName, tokenMint),
        custodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          tokenMint
        ),
      })
      .view()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  getAddLiquidityAmountAndFee = async (
    poolName: string,
    tokenMint: PublicKey,
    amount: BN
  ): Promise<AmountAndFee> => {
    return this.program.methods
      .getAddLiquidityAmountAndFee({
        amountIn: amount,
      })
      .accounts({
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
        custody: this.getCustodyKey(poolName, tokenMint),
        custodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          tokenMint
        ),
        lpTokenMint: this.getPoolLpTokenKey(poolName),
      })
      .remainingAccounts(await this.getCustodyMetas(poolName))
      .view()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  getRemoveLiquidityAmountAndFee = async (
    poolName: string,
    tokenMint: PublicKey,
    lpAmount: BN
  ): Promise<AmountAndFee> => {
    return this.program.methods
      .getRemoveLiquidityAmountAndFee({
        lpAmountIn: lpAmount,
      })
      .accounts({
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
        custody: this.getCustodyKey(poolName, tokenMint),
        custodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          tokenMint
        ),
        lpTokenMint: this.getPoolLpTokenKey(poolName),
      })
      .remainingAccounts(await this.getCustodyMetas(poolName))
      .view()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  getEntryPriceAndFee = async (
    poolName: string,
    tokenMint: PublicKey,
    collateralMint: PublicKey,
    collateral: BN,
    size: BN,
    side: PositionSide
  ): Promise<NewPositionPricesAndFee> => {
    return this.program.methods
      .getEntryPriceAndFee({
        collateral,
        size,
        side: side === "long" ? { long: {} } : { short: {} },
      })
      .accounts({
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
        custody: this.getCustodyKey(poolName, tokenMint),
        custodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          tokenMint
        ),
        collateralCustody: this.getCustodyKey(poolName, collateralMint),
        collateralCustodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          collateralMint
        ),
      })
      .view()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  getExitPriceAndFee = async (
    wallet: PublicKey,
    poolName: string,
    tokenMint: PublicKey,
    side: PositionSide
  ): Promise<PriceAndFee> => {
    return this.program.methods
      .getExitPriceAndFee({})
      .accounts({
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
        position: this.getPositionKey(wallet, poolName, tokenMint, side),
        custody: this.getCustodyKey(poolName, tokenMint),
        custodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          tokenMint
        ),
      })
      .view()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  getLiquidationPrice = async (
    wallet: PublicKey,
    poolName: string,
    tokenMint: PublicKey,
    collateralMint: PublicKey,
    side: PositionSide,
    addCollateral: BN,
    removeCollateral: BN
  ): Promise<BN> => {
    return this.program.methods
      .getLiquidationPrice({
        addCollateral,
        removeCollateral,
      })
      .accounts({
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
        position: this.getPositionKey(wallet, poolName, tokenMint, side),
        custody: this.getCustodyKey(poolName, tokenMint),
        custodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          tokenMint
        ),
        collateralCustody: this.getCustodyKey(poolName, collateralMint),
        collateralCustodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          collateralMint
        ),
      })
      .view()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  getLiquidationState = async (
    wallet: PublicKey,
    poolName: string,
    tokenMint: PublicKey,
    collateralMint: PublicKey,
    side: PositionSide
  ): Promise<number> => {
    return this.program.methods
      .getLiquidationState({})
      .accounts({
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
        position: this.getPositionKey(wallet, poolName, tokenMint, side),
        custody: this.getCustodyKey(poolName, tokenMint),
        custodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          tokenMint
        ),
        collateralCustody: this.getCustodyKey(poolName, collateralMint),
        collateralCustodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          collateralMint
        ),
      })
      .view()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  getPnl = async (
    wallet: PublicKey,
    poolName: string,
    tokenMint: PublicKey,
    collateralMint: PublicKey,
    side: PositionSide
  ): Promise<ProfitAndLoss> => {
    return this.program.methods
      .getPnl({})
      .accounts({
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
        position: this.getPositionKey(wallet, poolName, tokenMint, side),
        custody: this.getCustodyKey(poolName, tokenMint),
        custodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          tokenMint
        ),
        collateralCustody: this.getCustodyKey(poolName, collateralMint),
        collateralCustodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          collateralMint
        ),
      })
      .view()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  getSwapAmountAndFees = async (
    poolName: string,
    tokenMintIn: PublicKey,
    tokenMintOut: PublicKey,
    amountIn: BN
  ): Promise<SwapAmountAndFees> => {
    return this.program.methods
      .getSwapAmountAndFees({
        amountIn,
      })
      .accounts({
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
        receivingCustody: this.getCustodyKey(poolName, tokenMintIn),
        receivingCustodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          tokenMintIn
        ),
        dispensingCustody: this.getCustodyKey(poolName, tokenMintOut),
        dispensingCustodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          tokenMintOut
        ),
      })
      .view()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  getAum = async (poolName: string): Promise<BN> => {
    return this.program.methods
      .getAssetsUnderManagement({})
      .accounts({
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
      })
      .remainingAccounts(await this.getCustodyMetas(poolName))
      .view()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };
}
