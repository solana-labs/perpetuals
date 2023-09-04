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
} from "@solana/web3.js";
import { getAssociatedTokenAddress, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { sha256 } from "js-sha256";
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

export class PerpetualsClient {
  provider: AnchorProvider;
  program: Program<Perpetuals>;
  admin: Keypair;

  // pdas
  multisig: { publicKey: PublicKey; bump: number };
  authority: { publicKey: PublicKey; bump: number };
  perpetuals: { publicKey: PublicKey; bump: number };

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

    BN.prototype.toJSON = function () {
      return this.toString(10);
    };
  }

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
    return this.findProgramAddress("pool", name).publicKey;
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
    return Buffer.from(sha256.digest(`account:${name}`)).slice(0, 8);
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

  init = async (admins: PublicKey[], config: InitParams): Promise<void> => {
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

    await this.program.methods
      .init(config)
      .accounts({
        upgradeAuthority: this.provider.wallet.publicKey,
        multisig: this.multisig.publicKey,
        transferAuthority: this.authority.publicKey,
        perpetuals: this.perpetuals.publicKey,
        perpetualsProgram: this.program.programId,
        perpetualsProgramData,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .remainingAccounts(adminMetas)
      .rpc()
      .catch((err) => {
        console.error(err);
        throw err;
      });
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

    await this.program.methods
      .addLiquidity({ amountIn, minLpAmountOut })
      .accounts({
        owner: this.provider.wallet.publicKey,
        fundingAccount: await getAssociatedTokenAddress(
          tokenMint,
          this.provider.wallet.publicKey
        ),
        lpTokenAccount: await getAssociatedTokenAddress(
          lpTokenMint,
          this.provider.wallet.publicKey
        ),
        transferAuthority: this.authority.publicKey,
        perpetuals: this.perpetuals.publicKey,
        pool: this.getPoolKey(poolName),
        custody: this.getCustodyKey(poolName, tokenMint),
        custodyOracleAccount: await this.getCustodyOracleAccountKey(
          poolName,
          tokenMint
        ),
        custodyTokenAccount: this.getCustodyTokenAccountKey(
          poolName,
          tokenMint
        ),
        lpTokenMint,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .remainingAccounts(await this.getCustodyMetas(poolName))
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
