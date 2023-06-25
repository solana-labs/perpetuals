# Synthetic assets overview

Synthetics refer to a class of blockchain assets that mimic the value of another asset or financial instrument. They can represent real-world assets (like commodities, stocks, bonds, forex currencies, etc.) or other cryptocurrencies.

The main idea behind synthetic assets is to allow users to gain exposure to a wide range of assets without actually owning them. For example, a synthetic asset could track the price of gold, allowing users to speculate on the price of gold without having to buy, store, and sell physical gold.

There are several advantages to synthetic assets. For one, they can provide access to markets that might otherwise be difficult or expensive to enter. They can also be used to create complex financial products, like derivatives, that can be used for hedging or speculation. Synthetic assets have the potential to greatly increase the liquidity and accessibility of various markets, as they allow anyone with an internet connection to speculate on the price of a wide range of assets.

Synthetic assets are managed by Perpetuals smart-contract, which ensures transparency, security, and trustless transactions. Positions are collateralized by other cryptocurrencies, meaning that users must deposit a certain amount of cryptocurrency in order to open long or short position in a synthetic asset.

# Implementing Forex protocol with Synthetic assets

One of the potential use cases for the Synthetic assets is a decentralized Forex platform. Forex, short for foreign exchange, refers to the global marketplace for buying and selling currencies. It's the largest and most liquid financial market in the world, with trading volumes exceeding $7 trillion per day. Forex trading involves the simultaneous buying of one currency and selling of another. This is done in pairs, such as the Euro and the US Dollar (EUR/USD), or the British Pound and the US Dollar (GBP/USD), etc. The exchange rate between the two currencies determines the price of a forex pair.

Forex trading is typically done through a forex broker or a financial institution. Traders can take advantage of fluctuations in exchange rates to make profits. However, it's important to note that not all forex brokers are reputable, and there is a risk of fraud or unethical practices. This can include things like manipulation of prices, slippage, limiting withdrawals, etc. Even legitimate retail forex brokers process orders in-house. In other words, orders placed by traders are not visible anywhere other than the broker’s trading platform. There isn’t any external liquidity pool. In this case, there is a clear conflict of interest since the broker serves not only as an intermediary, but also as a counterparty to the transaction. The profits of the trader are equivalent to the losses of the broker.

On the other hand, decentralized Forex exchange that is implemented as a trustless and permissionless smart-contract doesn't have trading slippage (besides fees), requoting, or manipulations of any kind. It also inherits all benefits of a decentralized trading platform:

**Custody of Assets**: You retain control of your cryptocurrency at all times. You can open or close positions any time and withdraw profits.

**Permissionless and Trustless**: You don't need to trust the other party in your trade or ask anyone's permission to make a trade.

**Variety of Assets**: Synthetic assets allow to list any currency for trading as long as price quotes are supplied via Oracle or by protocol. This can provide more opportunities for trading and investing.

**Transparency**: All transactions are recorded on the Solana blockchain, providing a high level of transparency. You can verify transactions independently and be sure that trades are executed as specified by the smart contract.

**24x7 trading**: While traditional Forex markets operate 5 days a week, decentralized platform is always open for trading.

**Liquidity Provisioning**: Anyone can supply liquidity to the protocol and receive return on the investment that is generated from trading fees and borrow rate payments and is shared pro-rata between all liquidity providers.

## Initializing a perpetuals exchange

Reference implementation of the perpetuals exchange that can be found [here](https://github.com/solana-labs/perpetuals) supports synthetic assets and can be used to launch a Forex marketplace. It supports both, spot and leveraged trading. Follow the Quick start guide to build, deploy, and initialize the exchange.

## Initializing Forex markets

The primary distinction between real and synthetic assets lies in the way token custodies are set up. If upon creation `-v` (is_virtual) flag is specified, it enables a special (synthetic asset) mode for the custody. In this mode, deposits, withdrawals, or swaps are not permitted for such custody, and both long and short positions will require stablecoin collateral. This mode also relies on stablecoin reserves to cover potential profit payoffs and collect borrow interest. A custody can't be a stablecoin and virtual custody simultaneously, so in token pairs like EUR/USD, quote token should be a real stablecoin/collateral custody, for e.g. USDC. There could be multiple stablecoins custodies backing positions in the single base currency. The decision of which one to utilize for a particular trade rests with the front end or the trader.

Token custodies need to be initialized for every collateral or trading token. These custodies could be part of a single market (pool) or several markets. If all custodies are incorporated into a single pool, liquidity is shared among all currency pairs, providing benefits for traders. However, liquidity providers, who bear the profit and loss impact of traders for the market they supply liquidity to, may opt to limit their risk exposure to a single currency pair. In such scenarios, separate markets are considered more favorable.

## Supplying Oracle prices

Both spot and perpetual trades are executed based on oracle prices, ensuring a consistent execution price for traders, irrespective of their trade size. For synthetic assets to function properly, an oracle price needs to be obtained either from a service like Pyth, or custom price data must be provided by the protocol itself. For the latter scenario, the oracle type in the custody configuration needs to be set to `custom`, and the latest price and Exponential Moving Average (EMA) price of the asset should be updated as regularly as possible through the `setCustomOraclePrice` instruction.

In order to prevent front-running, defend against market manipulations, outdated Oracle price feed, and other problems, the protocol includes a range of checks and features:

- Entry, exit, and liquidation prices are calculated based on the lower of the two: oracle price or the EMA of the oracle price.
- Prices are cross-verified with a confidence interval to identify sudden, brief price fluctuations.
- A configurable spread per token can be used when EMA price is unavailable. This spread can be set to 2-3 standard deviations of price differences between oracle updates.
- There is a check for the last update time of the oracle price. This can be set to a minimal period to prevent the opening of positions using outdated prices.
