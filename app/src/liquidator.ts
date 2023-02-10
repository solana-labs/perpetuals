//@ts-nocheck
import { PublicKey } from "@solana/web3.js";
import { PerpetualsClient } from "./client";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function processLiquidations(client: PerpetualsClient) {
  // read all positions
  //
}

(async function main() {
  // read args
  if (process.argv.length != 4) {
    throw new Error(
      "Usage: npx ts-node app/src/liquidator.ts CLUSTER_URL POOL"
    );
  }
  let clusterUrl = process.argv[2];
  let pool = process.argv[3];
  let errorDelay = 10000;
  let liquidationDelay = 5000;

  // init client
  let client = new PerpetualsClient();
  while (true) {
    try {
      await client.init(cluster_url, tokenA, tokenB);
      break;
    } catch (err) {
      console.error(err);
      console.log(`Retrying in ${errorDelay} sec...`);
      await sleep(errorDelay);
    }
  }
  client.log("Initialized");

  // main loop
  while (true) {
    let perpetuals;
    try {
      perpetuals = await client.getPerpetuals();
    } catch (err) {
      console.error(err);
    }

    if (!perpetuals.permissions.allowClosePosition) {
      client.error(
        `Liquidations are not allowed at this time. Retrying in ${errorDelay} sec...`
      );
      await sleep(errorDelay);
      continue;
    }

    let [res, message] = await processLiquidations(client);
    if (res || message === "Nothing to liquidate at this time") {
      client.log(`Processed: ${message}`);
    } else {
      client.error(
        `Liquidation error: ${message}. Retrying in ${errorDelay} sec...`
      );
      await sleep(errorDelay);
      continue;
    }

    await sleep(liquidationDelay);
  }
})();
