//@ts-nocheck
import { BN } from "@project-serum/anchor";
import { PublicKey } from "@solana/web3.js";
import { PerpetualsClient } from "./client";

(async function main() {
  // read args
  if (process.argv.length < 5) {
    throw new Error(
      "Usage: npx ts-node src/cli.ts CLUSTER_URL ADMIN_KEY_PATH COMMAND [PARAMS]"
    );
  }
  let clusterUrl = process.argv[2];
  let adminKey = process.argv[3];
  let command = process.argv[4];
  let poolName = process.argv[5];
  let tokenMint;

  // constants and params, to be loaded from config files
  let perpetualsConfig = {
    minSignatures: 1,
    allowSwap: true,
    allowAddLiquidity: true,
    allowRemoveLiquidity: true,
    allowOpenPosition: true,
    allowClosePosition: true,
    allowPnlWithdrawal: true,
    allowCollateralWithdrawal: true,
    allowSizeChange: true,
  };
  let oracleConfig = {
    maxPriceError: new BN(10000),
    maxPriceAgeSec: 60,
    oracleType: { pyth: {} },
    oracleAccount: null,
  };
  let pricingConfig = {
    useEma: true,
    tradeSpreadLong: new BN(100),
    tradeSpreadShort: new BN(100),
    swapSpread: new BN(200),
    minInitialLeverage: new BN(10000),
    maxLeverage: new BN(1000000),
  };
  let permissions = {
    allowSwap: true,
    allowAddLiquidity: true,
    allowRemoveLiquidity: true,
    allowOpenPosition: true,
    allowClosePosition: true,
    allowPnlWithdrawal: true,
    allowCollateralWithdrawal: true,
    allowSizeChange: true,
  };
  let fees = {
    mode: { linear: {} },
    maxIncrease: new BN(20000),
    maxDecrease: new BN(5000),
    swap: new BN(100),
    addLiquidity: new BN(100),
    removeLiquidity: new BN(100),
    openPosition: new BN(100),
    closePosition: new BN(100),
    liquidation: new BN(100),
    protocolShare: new BN(10),
  };
  let ratios = {
    target: new BN(5000),
    min: new BN(10),
    max: new BN(20000),
  };

  // init client
  let client = new PerpetualsClient(clusterUrl, [adminKey]);
  client.log("Client Initialized");

  client.log("Processing command: " + command);
  switch (command) {
    case "init":
      await client.init(perpetualsConfig);
      client.prettyPrint(await client.getPerpetuals());
      break;
    case "addPool":
      await client.addPool(poolName);
      client.prettyPrint(await client.getPool(poolName));
      break;
    case "removePool":
      await client.removePool(poolName);
      break;
    case "addToken":
      tokenMint = new PublicKey(process.argv[6]);
      oracleConfig.oracleAccount = new PublicKey(process.argv[7]);
      await client.addToken(
        poolName,
        tokenMint,
        oracleConfig,
        pricingConfig,
        permissions,
        fees,
        ratios
      );
      client.prettyPrint(await client.getCustody(poolName, tokenMint));
      break;
    case "removeToken":
      await client.removeToken(poolName, tokenMint);
      break;
  }
})();
