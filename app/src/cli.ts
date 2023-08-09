/// Command-line interface for basic admin functions

import { BN } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import { PerpetualsClient } from "./client";
import { Command } from "commander";
import {
  BorrowRateParams,
  Fees,
  InitParams,
  OracleParams,
  Permissions,
  PositionSide,
  PricingParams,
  SetCustomOraclePriceParams,
} from "./types";

let client: PerpetualsClient;

function initClient(clusterUrl: string, adminKeyPath: string): void {
  process.env.ANCHOR_WALLET = adminKeyPath;
  client = new PerpetualsClient(clusterUrl, adminKeyPath);
  client.log("Client Initialized");
}

function init(adminSigners: PublicKey[], minSignatures: number): Promise<void> {
  // to be loaded from config file
  const perpetualsConfig: InitParams = {
    minSignatures: minSignatures,
    allowSwap: true,
    allowAddLiquidity: true,
    allowRemoveLiquidity: true,
    allowOpenPosition: true,
    allowClosePosition: true,
    allowPnlWithdrawal: true,
    allowCollateralWithdrawal: true,
    allowSizeChange: true,
  };

  return client.init(adminSigners, perpetualsConfig);
}

function setAuthority(
  adminSigners: PublicKey[],
  minSignatures: number
): Promise<void> {
  return client.setAdminSigners(adminSigners, minSignatures);
}

async function getMultisig(): Promise<void> {
  client.prettyPrint(await client.getMultisig());
}

async function getPerpetuals(): Promise<void> {
  client.prettyPrint(await client.getPerpetuals());
}

function addPool(poolName: string): Promise<void> {
  return client.addPool(poolName);
}

async function getPool(poolName: string): Promise<void> {
  client.prettyPrint(await client.getPool(poolName));
}

async function getPools(): Promise<void> {
  client.prettyPrint(await client.getPools());
}

function removePool(poolName: string): Promise<void> {
  return client.removePool(poolName);
}

async function addCustody(
  poolName: string,
  tokenMint: PublicKey,
  tokenOracle: PublicKey,
  isStable: boolean,
  isVirtual: boolean,
  oracleType: keyof OracleParams["oracleType"] = "custom"
): Promise<void> {
  // to be loaded from config file
  const oracleConfig: OracleParams = {
    maxPriceError: new BN(10_000),
    maxPriceAgeSec: 60,
    oracleType: { [oracleType]: {} },
    oracleAccount: tokenOracle,
  };

  const pricingConfig: PricingParams = {
    useEma: true,
    useUnrealizedPnlInAum: true,
    tradeSpreadLong: new BN(100),
    tradeSpreadShort: new BN(100),
    swapSpread: new BN(200),
    minInitialLeverage: new BN(10_000),
    maxInitialLeverage: new BN(1_000_000),
    maxLeverage: new BN(1_000_000),
    maxPayoffMult: new BN(10_000),
    maxUtilization: new BN(10_000),
    maxPositionLockedUsd: new BN(1_000_000_000),
    maxTotalLockedUsd: new BN(1_000_000_000),
  };
  const permissions: Permissions = {
    allowSwap: true,
    allowAddLiquidity: true,
    allowRemoveLiquidity: true,
    allowOpenPosition: true,
    allowClosePosition: true,
    allowPnlWithdrawal: true,
    allowCollateralWithdrawal: true,
    allowSizeChange: true,
  };
  const fees: Fees = {
    mode: { linear: {} },
    ratioMult: new BN(20_000),
    utilizationMult: new BN(20_000),
    swapIn: new BN(100),
    swapOut: new BN(100),
    stableSwapIn: new BN(100),
    stableSwapOut: new BN(100),
    addLiquidity: new BN(100),
    removeLiquidity: new BN(100),
    openPosition: new BN(100),
    closePosition: new BN(100),
    liquidation: new BN(100),
    protocolShare: new BN(10),
  };
  const borrowRate: BorrowRateParams = {
    baseRate: new BN(0),
    slope1: new BN(80_000),
    slope2: new BN(120_000),
    optimalUtilization: new BN(800_000_000),
  };

  const pool = await client.getPool(poolName);
  pool.ratios.push({
    target: new BN(5_000),
    min: new BN(10),
    max: new BN(10_000),
  });

  const ratios = client.adjustTokenRatios(pool.ratios);

  return client.addCustody(
    poolName,
    tokenMint,
    isStable,
    isVirtual,
    oracleConfig,
    pricingConfig,
    permissions,
    fees,
    borrowRate,
    ratios
  );
}

async function getCustody(
  poolName: string,
  tokenMint: PublicKey
): Promise<void> {
  client.prettyPrint(await client.getCustody(poolName, tokenMint));
}

async function getCustodies(poolName: string): Promise<void> {
  client.prettyPrint(await client.getCustodies(poolName));
}

async function removeCustody(
  poolName: string,
  tokenMint: PublicKey
): Promise<void> {
  const pool = await client.getPool(poolName);

  pool.ratios.pop();

  const ratios = client.adjustTokenRatios(pool.ratios);

  return client.removeCustody(poolName, tokenMint, ratios);
}

function upgradeCustody(poolName: string, tokenMint: PublicKey): Promise<void> {
  return client.upgradeCustody(poolName, tokenMint);
}

function setCustomOraclePrice(
  poolName: string,
  tokenMint: PublicKey,
  price: number,
  exponent: number,
  confidence: number,
  ema: number
): Promise<void> {
  const priceConfig: SetCustomOraclePriceParams = {
    price: new BN(price),
    expo: exponent,
    conf: new BN(confidence),
    ema: new BN(ema),
    publishTime: new BN(client.getTime()),
  };

  return client.setCustomOraclePrice(poolName, tokenMint, priceConfig);
}

function addLiquidity(
  poolName: string,
  tokenMint: PublicKey,
  amountIn: number,
  minLpAmountOut: number
): Promise<void> {
  return client.addLiquidity(
    poolName,
    tokenMint,
    new BN(amountIn),
    new BN(minLpAmountOut)
  );
}

function openPosition(
  poolName: string,
  tokenMint: PublicKey,
  collateralMint: PublicKey,
  side: PositionSide,
  price: number,
  collateral: number,
  size: number
): Promise<void> {
  return client.openPosition(
    poolName,
    tokenMint,
    collateralMint,
    side,
    new BN(price),
    new BN(collateral),
    new BN(size)
  );
}

async function getUserPosition(
  wallet: PublicKey,
  poolName: string,
  tokenMint: PublicKey,
  side: PositionSide
): Promise<void> {
  client.prettyPrint(
    await client.getUserPosition(wallet, poolName, tokenMint, side)
  );
}

async function getUserPositions(wallet: PublicKey): Promise<void> {
  client.prettyPrint(await client.getUserPositions(wallet));
}

async function getPoolTokenPositions(
  poolName: string,
  tokenMint: PublicKey
): Promise<void> {
  client.prettyPrint(await client.getPoolTokenPositions(poolName, tokenMint));
}

async function getAllPositions(): Promise<void> {
  client.prettyPrint(await client.getAllPositions());
}

async function getAddLiquidityAmountAndFee(
  poolName: string,
  tokenMint: PublicKey,
  amount: BN
): Promise<void> {
  client.prettyPrint(
    await client.getAddLiquidityAmountAndFee(poolName, tokenMint, amount)
  );
}

async function getRemoveLiquidityAmountAndFee(
  poolName: string,
  tokenMint: PublicKey,
  lpAmount: BN
): Promise<void> {
  client.prettyPrint(
    await client.getRemoveLiquidityAmountAndFee(poolName, tokenMint, lpAmount)
  );
}

async function getEntryPriceAndFee(
  poolName: string,
  tokenMint: PublicKey,
  collateralMint: PublicKey,
  collateral: BN,
  size: BN,
  side: PositionSide
): Promise<void> {
  client.prettyPrint(
    await client.getEntryPriceAndFee(
      poolName,
      tokenMint,
      collateralMint,
      collateral,
      size,
      side
    )
  );
}

async function getExitPriceAndFee(
  wallet: PublicKey,
  poolName: string,
  tokenMint: PublicKey,
  side: PositionSide
): Promise<void> {
  client.prettyPrint(
    await client.getExitPriceAndFee(wallet, poolName, tokenMint, side)
  );
}

async function getOraclePrice(
  poolName: string,
  tokenMint: PublicKey,
  useEma: boolean
): Promise<void> {
  client.prettyPrint(await client.getOraclePrice(poolName, tokenMint, useEma));
}

function getCustomOracleAccount(poolName: string, tokenMint: PublicKey): void {
  client.prettyPrint(
    client.getCustodyCustomOracleAccountKey(poolName, tokenMint)
  );
}

function getLpTokenMint(poolName: string): void {
  client.prettyPrint(client.getPoolLpTokenKey(poolName));
}

async function getLiquidationPrice(
  wallet: PublicKey,
  poolName: string,
  tokenMint: PublicKey,
  side: PositionSide,
  addCollateral: BN,
  removeCollateral: BN
): Promise<void> {
  client.prettyPrint(
    await client.getLiquidationPrice(
      wallet,
      poolName,
      tokenMint,
      await client.getCollateralCustodyMint(wallet, poolName, tokenMint, side),
      side,
      addCollateral,
      removeCollateral
    )
  );
}

async function getLiquidationState(
  wallet: PublicKey,
  poolName: string,
  tokenMint: PublicKey,
  side: PositionSide
): Promise<void> {
  client.prettyPrint(
    await client.getLiquidationState(
      wallet,
      poolName,
      tokenMint,
      await client.getCollateralCustodyMint(wallet, poolName, tokenMint, side),
      side
    )
  );
}

async function getPnl(
  wallet: PublicKey,
  poolName: string,
  tokenMint: PublicKey,
  side: PositionSide
): Promise<void> {
  client.prettyPrint(
    await client.getPnl(
      wallet,
      poolName,
      tokenMint,
      await client.getCollateralCustodyMint(wallet, poolName, tokenMint, side),
      side
    )
  );
}

async function getSwapAmountAndFees(
  poolName: string,
  tokenMintIn: PublicKey,
  tokenMintOut: PublicKey,
  amountIn: BN
): Promise<void> {
  client.prettyPrint(
    await client.getSwapAmountAndFees(
      poolName,
      tokenMintIn,
      tokenMintOut,
      amountIn
    )
  );
}

async function getAum(poolName: string): Promise<void> {
  client.prettyPrint(await client.getAum(poolName));
}

(async function main() {
  const program = new Command();
  program
    .name("cli.ts")
    .description("CLI to Solana Perpetuals Exchange Program")
    .version("0.1.0")
    .option(
      "-u, --url <string>",
      "URL for Solana's JSON RPC",
      "https://api.devnet.solana.com"
    )
    .requiredOption("-k, --keypair <path>", "Filepath to the admin keypair")
    .hook("preSubcommand", (thisCommand, subCommand) => {
      if (!program.opts().keypair) {
        throw Error("required option '-k, --keypair <path>' not specified");
      }
      initClient(program.opts().url, program.opts().keypair);
      client.log(`Processing command '${thisCommand.args[0]}'`);
    })
    .hook("postAction", () => {
      client.log("Done");
    });

  program
    .command("init")
    .description("Initialize the on-chain program")
    .requiredOption("-m, --min-signatures <int>", "Minimum signatures")
    .argument("<pubkey...>", "Admin public keys")
    .action(async (args, options) => {
      await init(
        args.map((x) => new PublicKey(x)),
        options.minSignatures
      );
    });

  program
    .command("set-authority")
    .description("Set protocol admins")
    .requiredOption("-m, --min-signatures <int>", "Minimum signatures")
    .argument("<pubkey...>", "Admin public keys")
    .action(async (args, options) => {
      await setAuthority(
        args.map((x) => new PublicKey(x)),
        options.minSignatures
      );
    });

  program
    .command("get-multisig")
    .description("Print multisig state")
    .action(async () => {
      await getMultisig();
    });

  program
    .command("get-perpetuals")
    .description("Print perpetuals global state")
    .action(async () => {
      await getPerpetuals();
    });

  program
    .command("add-pool")
    .description("Create a new pool")
    .argument("<string>", "Pool name")
    .action(async (poolName) => {
      await addPool(poolName);
    });

  program
    .command("get-pool")
    .description("Print metadata for the pool")
    .argument("<string>", "Pool name")
    .action(async (poolName) => {
      await getPool(poolName);
    });

  program
    .command("get-pools")
    .description("Print metadata for all pools")
    .action(async () => {
      await getPools();
    });

  program
    .command("remove-pool")
    .description("Remove the pool")
    .argument("<string>", "Pool name")
    .action(async (poolName) => {
      await removePool(poolName);
    });

  program
    .command("add-custody")
    .description("Add a new token custody to the pool")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .argument("<pubkey>", "Token oracle account")
    .option("-s, --stablecoin", "Stablecoin custody")
    .option("-v, --virtual", "Virtual asset custody")
    .option("-t, --oracletype <string>", "Oracle type (pyth, none, custom)")
    .action(async (poolName, tokenMint, tokenOracle, options) => {
      await addCustody(
        poolName,
        new PublicKey(tokenMint),
        new PublicKey(tokenOracle),
        options.stablecoin,
        options.virtual,
        options.oracletype
      );
    });

  program
    .command("get-custody")
    .description("Print metadata for the token custody")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .action(async (poolName, tokenMint) => {
      await getCustody(poolName, new PublicKey(tokenMint));
    });

  program
    .command("get-custodies")
    .description("Print metadata for all custodies")
    .argument("<string>", "Pool name")
    .action(async (poolName) => {
      await getCustodies(poolName);
    });

  program
    .command("remove-custody")
    .description("Remove the token custody from the pool")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .action(async (poolName, tokenMint) => {
      await removeCustody(poolName, new PublicKey(tokenMint));
    });

  program
    .command("upgrade-custody")
    .description("Upgrade deprecated custody to the new version")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .action(async (poolName, tokenMint, options) => {
      await upgradeCustody(poolName, new PublicKey(tokenMint));
    });

  program
    .command("set-oracle-price")
    .description("Set custom oracle price")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .requiredOption("-p, --price <int>", "Current price as integer")
    .requiredOption("-e, --exponent <int>", "Price exponent")
    .requiredOption("-c, --confidence <int>", "Confidence")
    .requiredOption("-m, --ema <int>", "EMA price as integer")
    .action(async (poolName, tokenMint, options) => {
      await setCustomOraclePrice(
        poolName,
        new PublicKey(tokenMint),
        options.price,
        options.exponent,
        options.confidence,
        options.ema
      );
    });

  program
    .command("add-liquidity")
    .description("Deposit liquidity to the custody")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .requiredOption("-i, --amount-in <int>", "Amount to deposit")
    .requiredOption(
      "-o, --min-amount-out <int>",
      "Minimum LP amount to receive"
    )
    .action(async (poolName, tokenMint, options) => {
      await addLiquidity(
        poolName,
        new PublicKey(tokenMint),
        options.amountIn,
        options.minAmountOut
      );
    });

  program
    .command("open-position")
    .description("Open a new perpetuals position")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .argument("<pubkey>", "Collateral mint")
    .argument("<string>", "Position side (long / short)")
    .requiredOption("-p, --price <int>", "Entry price")
    .requiredOption("-c, --collateral <int>", "Collateral amount")
    .requiredOption("-s, --size <int>", "Position size")
    .action(async (poolName, tokenMint, collateralMint, side, options) => {
      await openPosition(
        poolName,
        new PublicKey(tokenMint),
        new PublicKey(collateralMint),
        side,
        options.price,
        options.collateral,
        options.size
      );
    });

  program
    .command("get-user-position")
    .description("Print user position metadata")
    .argument("<pubkey>", "User wallet")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .argument("<string>", "Position side (long / short)")
    .action(async (wallet, poolName, tokenMint, side) => {
      await getUserPosition(
        new PublicKey(wallet),
        poolName,
        new PublicKey(tokenMint),
        side
      );
    });

  program
    .command("get-user-positions")
    .description("Print all user positions")
    .argument("<pubkey>", "User wallet")
    .action(async (wallet) => {
      await getUserPositions(new PublicKey(wallet));
    });

  program
    .command("get-pool-token-positions")
    .description("Print positions in the token")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .action(async (poolName, tokenMint) => {
      await getPoolTokenPositions(poolName, new PublicKey(tokenMint));
    });

  program
    .command("get-all-positions")
    .description("Print all open positions")
    .action(async () => {
      await getAllPositions();
    });

  program
    .command("get-add-liquidity-amount-and-fee")
    .description("Compute LP amount returned and fee for add liquidity")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .requiredOption("-a, --amount <bigint>", "Token amount")
    .action(async (poolName, tokenMint, options) => {
      await getAddLiquidityAmountAndFee(
        poolName,
        new PublicKey(tokenMint),
        new BN(options.amount)
      );
    });

  program
    .command("get-remove-liquidity-amount-and-fee")
    .description("Compute token amount returned and fee for remove liquidity")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .requiredOption("-a, --amount <bigint>", "LP token amount")
    .action(async (poolName, tokenMint, options) => {
      await getRemoveLiquidityAmountAndFee(
        poolName,
        new PublicKey(tokenMint),
        new BN(options.amount)
      );
    });

  program
    .command("get-entry-price-and-fee")
    .description("Compute price and fee to open a position")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .argument("<pubkey>", "Collateral mint")
    .argument("<string>", "Position side (long / short)")
    .requiredOption("-c, --collateral <bigint>", "Collateral")
    .requiredOption("-s, --size <bigint>", "Size")
    .action(async (poolName, tokenMint, collateralMint, side, options) => {
      await getEntryPriceAndFee(
        poolName,
        new PublicKey(tokenMint),
        new PublicKey(collateralMint),
        new BN(options.collateral),
        new BN(options.size),
        side
      );
    });

  program
    .command("get-exit-price-and-fee")
    .description("Compute price and fee to close the position")
    .argument("<pubkey>", "User wallet")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .argument("<string>", "Position side (long / short)")
    .action(async (wallet, poolName, tokenMint, side) => {
      await getExitPriceAndFee(
        new PublicKey(wallet),
        poolName,
        new PublicKey(tokenMint),
        side
      );
    });

  program
    .command("get-oracle-price")
    .description("Read oracle price for the token")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .option("-e, --ema", "Return EMA price")
    .action(async (poolName, tokenMint, options) => {
      await getOraclePrice(poolName, new PublicKey(tokenMint), options.ema);
    });

  program
    .command("get-custom-oracle-account")
    .description("Get custom oracle account address for the token")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .action(async (poolName, tokenMint, options) => {
      await getCustomOracleAccount(poolName, new PublicKey(tokenMint));
    });

  program
    .command("get-lp-token-mint")
    .description("Get LP token mint address for the pool")
    .argument("<string>", "Pool name")
    .action(async (poolName, options) => {
      await getLpTokenMint(poolName);
    });

  program
    .command("get-liquidation-price")
    .description("Compute liquidation price for the position")
    .argument("<pubkey>", "User wallet")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .argument("<string>", "Position side (long / short)")
    .option("-a, --add-collateral <bigint>", "Collateral to add")
    .option("-r, --remove-collateral <bigint>", "Collateral to remove")
    .action(async (wallet, poolName, tokenMint, side, options) => {
      await getLiquidationPrice(
        new PublicKey(wallet),
        poolName,
        new PublicKey(tokenMint),
        side,
        new BN(options.addCollateral),
        new BN(options.removeCollateral)
      );
    });

  program
    .command("get-liquidation-state")
    .description("Get liquidation state of the position")
    .argument("<pubkey>", "User wallet")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .argument("<string>", "Position side (long / short)")
    .action(async (wallet, poolName, tokenMint, side) => {
      await getLiquidationState(
        new PublicKey(wallet),
        poolName,
        new PublicKey(tokenMint),
        side
      );
    });

  program
    .command("get-pnl")
    .description("Compute PnL of the position")
    .argument("<pubkey>", "User wallet")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint")
    .argument("<string>", "Position side (long / short)")
    .action(async (wallet, poolName, tokenMint, side) => {
      await getPnl(
        new PublicKey(wallet),
        poolName,
        new PublicKey(tokenMint),
        side
      );
    });

  program
    .command("get-swap-amount-and-fees")
    .description("Compute amount out and fees for the swap")
    .argument("<string>", "Pool name")
    .argument("<pubkey>", "Token mint in")
    .argument("<pubkey>", "Token mint out")
    .requiredOption("-i, --amount-in <bigint>", "Token amount to be swapped")
    .action(async (poolName, tokenMintIn, tokenMintOut, options) => {
      await getSwapAmountAndFees(
        poolName,
        new PublicKey(tokenMintIn),
        new PublicKey(tokenMintOut),
        new BN(options.amountIn)
      );
    });

  program
    .command("get-aum")
    .description("Get assets under management")
    .argument("<string>", "Pool name")
    .action(async (poolName) => {
      await getAum(poolName);
    });

  await program.parseAsync(process.argv);

  if (!process.argv.slice(2).length) {
    program.outputHelp();
  }
})();
