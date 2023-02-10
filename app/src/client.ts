//@ts-nocheck
import {
  setProvider,
  Program,
  AnchorProvider,
  workspace,
  utils,
  BN,
} from "@project-serum/anchor";
import { Perpetuals } from "../../target/types/perpetuals";
import {
  PublicKey,
  TransactionInstruction,
  Transaction,
  SystemProgram,
  AccountMeta,
  Keypair,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import {
  getAccount,
  getAssociatedTokenAddress,
  createAssociatedTokenAccountInstruction,
  createCloseAccountInstruction,
  createSyncNativeInstruction,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import JSBI from "jsbi";
import fetch from "node-fetch";
import { sha256 } from "js-sha256";
import { encode } from "bs58";
import { readFileSync } from "fs";

export type PositionSide = "long" | "short";

export class PerpetualsClient {
  provider: AnchorProvider;
  program: Program<Perpetuals>;

  admins: Keypair[];
  adminMetas: AccountMeta[];

  // pdas
  multisig: { publicKey: PublicKey; bump: number };
  authority: { publicKey: PublicKey; bump: number };
  perpetuals: { publicKey: PublicKey; bump: number };

  constructor(clusterUrl: string, adminKeys: string[]) {
    this.provider = AnchorProvider.local(clusterUrl, {
      commitment: "confirmed",
      preflightCommitment: "confirmed",
    });
    setProvider(this.provider);
    this.program = workspace.Perpetuals as Program<Perpetuals>;

    this.admins = [];
    this.adminMetas = [];
    for (const adminKey of adminKeys) {
      this.admins.push(
        Keypair.fromSecretKey(
          new Uint8Array(JSON.parse(readFileSync(adminKey).toString()))
        )
      );
      this.adminMetas.push({
        isSigner: false,
        isWritable: false,
        pubkey: this.admins.at(-1).publicKey,
      });
    }

    this.multisig = this.findProgramAddress("multisig");
    this.authority = this.findProgramAddress("transfer_authority");
    this.perpetuals = this.findProgramAddress("perpetuals");

    BN.prototype.toJSON = function () {
      return this.toString(10);
    };
  }

  findProgramAddress = (label: string, extraSeeds = null) => {
    let seeds = [Buffer.from(utils.bytes.utf8.encode(label))];
    if (extraSeeds) {
      for (let extraSeed of extraSeeds) {
        if (typeof extraSeed === "string") {
          seeds.push(Buffer.from(utils.bytes.utf8.encode(extraSeed)));
        } else if (Array.isArray(extraSeed)) {
          seeds.push(Buffer.from(extraSeed));
        } else {
          seeds.push(extraSeed.toBuffer());
        }
      }
    }
    let res = PublicKey.findProgramAddressSync(seeds, this.program.programId);
    return { publicKey: res[0], bump: res[1] };
  };

  getPerpetuals = async () => {
    return this.program.account.perpetuals.fetch(this.perpetuals.publicKey);
  };

  getPoolKey = (name: string) => {
    return this.findProgramAddress("pool", name).publicKey;
  };

  getPool = async (name: string) => {
    return this.program.account.pool.fetch(this.getPoolKey(name));
  };

  getPools = async () => {
    //return this.program.account.pool.all();
    let perpetuals = await this.getPerpetuals();
    return this.program.account.pool.fetchMultiple(perpetuals.pools);
  };

  getPoolLpTokenKey = (name: string) => {
    return this.findProgramAddress("lp_token_mint", [this.getPoolKey(name)])
      .publicKey;
  };

  getCustodyKey = (poolName: string, tokenMint: PublicKey) => {
    return this.findProgramAddress("custody", [
      this.getPoolKey(poolName),
      tokenMint,
    ]).publicKey;
  };

  getCustodyTokenAccountKey = (poolName: string, tokenMint: PublicKey) => {
    return this.findProgramAddress("custody_token_account", [
      this.getPoolKey(poolName),
      tokenMint,
    ]).publicKey;
  };

  getCustodyOracleAccountKey = async (
    poolName: string,
    tokenMint: PublicKey
  ) => {
    return (await this.getCustody(poolName, tokenMint)).oracle.oracleAccount;
  };

  getCustodyTestOracleAccountKey = (poolName: string, tokenMint: PublicKey) => {
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

  getCustodies = async (poolName: string) => {
    //return this.program.account.custody.all();
    let pool = await this.getPool(poolName);
    return this.program.account.custody.fetchMultiple(
      pool.tokens.map((t) => t.custody)
    );
  };

  getMultisig = async () => {
    return this.program.account.multisig.fetch(this.multisig.publicKey);
  };

  getPositionKey = (
    wallet: PublicKey,
    poolName: string,
    tokenMint: PublicKey,
    side: PositionSide
  ) => {
    let pool = this.getPoolKey(poolName);
    let custody = this.getCustodyKey(poolName, tokenMint);
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
    let data = encode(
      Buffer.concat([
        this.getAccountDiscriminator("Position"),
        wallet.toBuffer(),
      ])
    );
    let positions = await this.provider.connection.getProgramAccounts(
      this.program.programId,
      {
        filters: [{ dataSize: 152 }, { memcmp: { bytes: data, offset: 0 } }],
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

  getAccountDiscriminator = (name: string) => {
    return Buffer.from(sha256.digest(`account:${name}`)).slice(0, 8);
  };

  log = (...message: string) => {
    let date = new Date();
    let date_str = date.toDateString();
    let time = date.toLocaleTimeString();
    console.log(`[${date_str} ${time}] ${message}`);
  };

  prettyPrint = (object: object) => {
    console.log(JSON.stringify(object, null, 2));
  };

  ///////
  // instructions

  init = async (config) => {
    let perpetualsProgramData = PublicKey.findProgramAddressSync(
      [this.program.programId.toBuffer()],
      new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111")
    )[0];

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
      .remainingAccounts(this.adminMetas)
      .rpc()
      .catch((err) => {
        console.error(err);
        throw err;
      });
  };

  setAdminSigners = async (admins: Publickey[], minSignatures: number) => {
    let multisig = await this.program.account.multisig.fetch(
      this.multisig.publicKey
    );
    let adminMetas = [];
    for (const admin of admins) {
      adminMetas.push({
        isSigner: false,
        isWritable: false,
        pubkey: admin,
      });
    }
    for (let i = 0; i < multisig.minSignatures; ++i) {
      try {
        await this.program.methods
          .setAdminSigners({
            minSignatures,
          })
          .accounts({
            admin: this.admins[i].publicKey,
            multisig: this.multisig.publicKey,
          })
          .remainingAccounts(adminMetas)
          .signers([this.admins[i]])
          .rpc();
      } catch (err) {
        if (this.printErrors) {
          console.log(err);
        }
        throw err;
      }
    }
  };

  addPool = async (name: string) => {
    let multisig = await this.program.account.multisig.fetch(
      this.multisig.publicKey
    );
    for (let i = 0; i < multisig.minSignatures; ++i) {
      await this.program.methods
        .addPool({ name })
        .accounts({
          admin: this.admins[i].publicKey,
          multisig: this.multisig.publicKey,
          transferAuthority: this.authority.publicKey,
          perpetuals: this.perpetuals.publicKey,
          pool: this.getPoolKey(name),
          lpTokenMint: this.getPoolLpTokenKey(name),
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .signers([this.admins[i]])
        .rpc()
        .catch((err) => {
          console.error(err);
          throw err;
        });
    }
  };

  removePool = async (name: string) => {
    let multisig = await this.program.account.multisig.fetch(
      this.multisig.publicKey
    );
    for (let i = 0; i < multisig.minSignatures; ++i) {
      await this.program.methods
        .removePool({})
        .accounts({
          admin: this.admins[i].publicKey,
          multisig: this.multisig.publicKey,
          transferAuthority: this.authority.publicKey,
          perpetuals: this.perpetuals.publicKey,
          pool: this.getPoolKey(name),
          systemProgram: SystemProgram.programId,
        })
        .signers([this.admins[i]])
        .rpc()
        .catch((err) => {
          console.error(err);
          throw err;
        });
    }
  };

  addToken = async (
    poolName: string,
    tokenMint: PublicKey,
    oracleConfig,
    pricingConfig,
    permissions,
    fees,
    ratios
  ) => {
    let multisig = await this.program.account.multisig.fetch(
      this.multisig.publicKey
    );
    for (let i = 0; i < multisig.minSignatures; ++i) {
      await this.program.methods
        .addToken({
          oracle: oracleConfig,
          pricing: pricingConfig,
          permissions,
          fees,
          targetRatio: ratios.target,
          minRatio: ratios.min,
          maxRatio: ratios.max,
        })
        .accounts({
          admin: this.admins[i].publicKey,
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
        .signers([this.admins[i]])
        .rpc()
        .catch((err) => {
          console.error(err);
          throw err;
        });
    }
  };

  removeToken = async (poolName: string, tokenMint: PublicKey) => {
    let multisig = await this.program.account.multisig.fetch(
      this.multisig.publicKey
    );
    for (let i = 0; i < multisig.minSignatures; ++i) {
      await this.program.methods
        .removeToken({})
        .accounts({
          admin: this.admins[i].publicKey,
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
        .signers([this.admins[i]])
        .rpc()
        .catch((err) => {
          console.error(err);
          throw err;
        });
    }
  };

  getOraclePrice = async (
    poolName: string,
    tokenMint: PublicKey,
    ema: boolean
  ) => {
    return await this.program.methods
      .getOraclePrice({
        ema,
      })
      .accounts({
        signer: this.provider.wallet.publicKey,
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

  getEntryPriceAndFee = async (
    poolName: string,
    tokenMint: PublicKey,
    collateral: typeof BN,
    size: typeof BN,
    side: PositionSide
  ) => {
    return await this.program.methods
      .getEntryPriceAndFee({
        collateral,
        size,
        side: side === "long" ? { long: {} } : { short: {} },
      })
      .accounts({
        signer: this.provider.wallet.publicKey,
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

  getExitPriceAndFee = async (
    wallet: PublicKey,
    poolName: string,
    tokenMint: PublicKey,
    side: PositionSide
  ) => {
    return await this.program.methods
      .getExitPriceAndFee({})
      .accounts({
        signer: this.provider.wallet.publicKey,
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
    side: PositionSide
  ) => {
    return await this.program.methods
      .getLiquidationPrice({})
      .accounts({
        signer: this.provider.wallet.publicKey,
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

  getLiquidationState = async (
    wallet: PublicKey,
    poolName: string,
    tokenMint: PublicKey,
    side: PositionSide
  ) => {
    return await this.program.methods
      .getLiquidationState({})
      .accounts({
        signer: this.provider.wallet.publicKey,
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

  getPnl = async (
    wallet: PublicKey,
    poolName: string,
    tokenMint: PublicKey,
    side: PositionSide
  ) => {
    return await this.program.methods
      .getPnl({})
      .accounts({
        signer: this.provider.wallet.publicKey,
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

  getSwapAmountAndFees = async (
    poolName: string,
    tokenMintIn: PublicKey,
    tokenMintOut: PublicKey,
    amountIn: BN
  ) => {
    return await this.program.methods
      .getSwapAmountAndFees({
        amountIn,
      })
      .accounts({
        signer: this.provider.wallet.publicKey,
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
}
