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

  getCustodyOracleAccountKey = (poolName: string, tokenMint: PublicKey) => {
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

  log = (message: string) => {
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
}
