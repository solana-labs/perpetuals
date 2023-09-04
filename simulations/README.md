1. Overview and Set up
This simulation models a perpetuals protocol that includes liquidity providers, traders, and pools, and how they interact. The simulation uses cadCAD to model this complex system over time and under different conditions.

Setting up the venv:
$ python3.9 -m venv venv

Activate the venv (Unix)
$ source venv/bin/activate

Install requirements from requirements.txt
$ pip install -r requirements.txt

Use python 3.9.16
perputuals_simulation is the directory which contains the executable
To execute simulation use
$ python run.py

All of the simulations will be saved into runs directory

2. Components

2.1 Partial State Update Blocks
The simulation is divided into several partial state update blocks, each representing a distinct aspect of the protocol:

Liquidity (liquidity.py)
Policies: Dictates how liquidity providers decide to interact with the system.
Variables: Updates the system state concerning liquidity providers and the liquidity pools.
Trading (trading.py)
Policies: Defines the trading behavior of agents in the system.
Variables: Updates the system state concerning traders, pools, and related metrics.
Traction (traction.py)
Policies: Defines how new agents are generated and added to the system.
Variables: Updates the system state with more liquidity providers and traders.

2.2 System Parameters (sys_params.py)
Defines both protocol-specific parameters and simulation parameters. The protocol parameters include details like fees, liquidation thresholds, and maximum margins, while the simulation parameters dictate the behavior of agents in the system, such as the chance of trading and the traction of traders.

Initial Conditions (initial_conditions): These dictate the starting state of the system. The parameters you can set include:

genesis_traders: Initial number of traders in the system.
genesis_providers: Initial number of liquidity providers.
num_of_min: Number of minutes the simulation should run.
pool_fees: Fees associated with the liquidity pool.

System Parameters (sys_params): These represent both the protocol and the simulation behavior.

Protocol Parameters:
base_fee: Basic fee for certain operations.
ratio_mult: Multiplier for ratios.
max_margin: Maximum margin for assets.
liquidation_threshold: Thresholds for asset liquidation.
And others, like rate parameters, swap fees, and liquidity provider fees.
Simulation Parameters:
trader_traction: Percentage change in the number of traders.
lp_traction: Percentage change in the number of liquidity providers.
trade_chance: Probability values for long and short trades.
swap_chance: Probability values for swapping in and out tokens.
event: Specifies which event (scenario) the simulation should use.

2.3 State Variables (state_variables.py)
State variables are the backbone of the simulation. They represent the state of the system at any given timestep:

Traders (generate_traders function):

Each trader is initialized with a random amount of liquidity in different assets.
They have no initial long or short positions.
Other metrics like PnL, avg_position_hold, and risk_factor are also initialized.

Liquidity Providers (generate_providers function):

Each liquidity provider is given a random amount of funds in different assets.
Thresholds for adding and removing liquidity are set.
They have no initial share in the pool.

Pools (generate_pools function):

The pool starts with initial liquidity based on asset prices.
Various metrics like open interest (oi_long, oi_short), volume, total fees collected, and others are initialized.
The pool also has initial target ratios, minimum ratios, and maximum ratios for different assets.

Genesis States (genesis_states):

This is an array that collects the initial states for traders, liquidity providers, and pools for each event (scenario) in the simulation.

3. Flow of Simulation

Liquidity Phase: Liquidity providers might decide to add or remove liquidity based on their policies. The liquidity_policy in liquidity.py determines this behavior, and the system state is updated accordingly.
Trading Phase: Traders might decide to open/close positions or swap tokens. The trading_policy in trading.py defines this behavior. Updates to traders, pools, and related metrics are made.
Traction Phase: New agents (both traders and liquidity providers) might be added to the system. The behavior is defined in traction.py, and the state variables are updated.
The simulation runs for a specified number of timesteps, with each timestep consisting of the above three phases.

4. Behavior logic

4.1 Trading logic
The trading simulation mainly consists of three important functions: trading_decision, swap_decision, and trading_policy.

4.1.1 Trading Decision (trading_decision function trad_mech.py)
This function aims to simulate the decisions made by an individual trader within the system. It takes into account a multitude of variables like the trader's current financial state, the current timestep, inherent risk tolerance, and the asset being considered for trading.

Key Steps and Logic:
Asset Pricing:
cs_price, cl_price, os_price, and ol_price represent different price points for the asset being traded. They are extracted from the asset_pricing dictionary which contains the upper and lower pricing provided by the platform with the upside leaning towards the platform. Initial prices are provided by Pyth oracle.

Position Handling:
Checks if the trader has any open long or short positions that need to be closed based on their average position hold time (avg_position_hold) or if they meet the liquidation threshold.
PnL (Profit and Loss), interest, and payout are calculated to determine if liquidation is necessary.

Random Trading Decisions:
A random number (trade_action) is generated to decide if the trader will initiate a new trade.
Different checks are in place to ensure that the trader has sufficient liquidity in either the asset or a stablecoin (USDC/USDT) to open a new position.
The size of the new position (lot_size) is influenced by the trader's risk factor and the maximum available leverage (max_margin).

Interest Handling:
If the trader already has an open position in the same asset, interest is calculated based on the duration for which the position has been open.

4.1.2 Swap Decision (swap_decision function swap_mech.py)
This function determines if a trader will swap one asset for another. The function takes into account various factors such as the trader's existing liquidity in the asset, current asset prices, and the likelihood of a swap (swap_chance).

Key Steps and Logic:
Eligibility Check:
The function first checks if the asset is tradable (asset_prices[asset][2] == True). If not, the function returns None.

Random Swap Decision:
A random number (swap_action) is generated to decide whether the trader will perform a swap operation.
Buy Decision (swap_action < swap_chance[0]):

If the random number falls below swap_chance[0], the trader decides to buy a new asset.
A random amount of the existing asset (swap_in) is selected to be swapped.
A random target asset (swap_out_asset) is chosen.
The quantity of the target asset to be obtained (swap_out) is calculated based on current market prices.
Sell Decision (swap_action > swap_chance[1]):

If the random number is greater than swap_chance[1], the trader decides to sell the asset.
A random amount of the existing asset (swap_out) is selected to be swapped out.
A random target asset (swap_in_asset) is chosen.
The quantity of the target asset to be obtained (swap_in) is calculated based on current market prices.
Output:

The function returns a dictionary containing the details of the swap (swap_in and swap_out), or None if no swap is to be performed.
Example Output:
Buy decision: {'swap_in': [5, 'ETH'], 'swap_out': [15, 'USDC']}
Sell decision: {'swap_in': [20, 'USDC'], 'swap_out': [2, 'ETH']}
No swap: None

4.1.3 Trading Policy (trading_policy function trading.py)
This function orchestrates the trading decisions across all traders and liquidity pools in the system. It does so by calling the trading_decision function for each trader and asset pair.

Key Steps and Logic:

Iterating Over Pools and Traders:

For every pool and trader, the trading_decision function is called, and the decisions are stored.

Trade Execution:
The trade decisions returned by trading_decision are executed using helper functions like execute_long and execute_short.
These functions also update relevant metrics like fees, liquidations, and swaps.

Token Swapping:
The swap_decision function guides the decision-making process for token swaps. If a trader decides to perform a swap, the swap_tokens function is invoked to carry out the swap operation. The fees associated with the swap are then calculated using the swap_fee_calc function.

Metrics and State Updates:
The state of the traders, liquidity providers, and pools is updated based on the executed trades and swaps.
Metrics such as the number of longs, shorts, swaps, and liquidations are tallied and returned as part of the action dictionary.

Helper Functions:
execute_long: Responsible for opening or closing a long position based on the decisions made. It updates the trader's and pool's state accordingly.
execute_short: Similar to execute_long but for short positions.
swap_tokens: Executes the actual token swapping between different assets in the pool.
swap_fee_calc: Determines the fees associated with a token swap.
calculate_open_pnl: Updates the open PnL (Profit and Loss) for traders based on their open positions and the current asset prices.

4.2 Liquidity provisioning logic

4.2.1 Liquidity provisioning decision (liquidity_provider_decision Function liq_mech.py)
This function determines the amount of liquidity to be added or removed from the pool by a liquidity provider.

Key Steps and Logic:
Initialize Decision Variables:
A decision dictionary is initialized with asset names as keys and a default value of 0.

Calculate Pool Yield and Volatility Multipliers:
The function calculates an aggregate pool yield and volatility, weighted by the ratio of assets in the pool.

Threshold Adjustment:
The add and remove thresholds for each asset are dynamically adjusted based on the calculated volatility.

Liquidity Addition:
If the calculated yield exceeds the adjusted 'add_threshold', the liquidity provider considers adding liquidity.
The amount of liquidity to be added is proportional to the excess yield and the available funds.

Liquidity Removal:
Conversely, if the yield is below the 'remove_threshold', the liquidity provider considers removing liquidity.
The amount of liquidity to be removed is proportional to the shortfall in yield and the current liquidity.
Output:

The function returns a dictionary containing decisions about the amount of liquidity to add or remove for each asset.

4.2.2 Liquidity policy (liquidity_policy function liquidity.py)
This function updates the state variables to reflect the decisions made by each liquidity provider.

Key Steps and Logic:
Initialize State Variables:
State variables are initialized from the previous state.

Asset Volatility and Prices:
Asset volatility and prices are fetched for each pool.

Total Value Locked (TVL) and Pool Ratios:
The TVL and asset ratios for each pool are updated.

Liquidity Decisions:
For each liquidity provider, the liquidity_provider_decision function is invoked to get the liquidity provision decisions.

Constraints and Adjustments:
Various checks and constraints are applied to ensure that the liquidity provision decisions are viable.
Open positions and fees are also considered in the decision-making process.

State Update:
State variables are updated to reflect the new liquidity levels.

Output:
The function returns a dictionary containing the updated state variables.

5. Monte Carlo Runs and Execution

5.1 Execution (run.py)
This script simulates a system and extracts relevant metrics from the simulation.

Functions:
run(event):
Purpose: Executes the simulation for a specific event.
Arguments:
event: Index of the event/scenario to run the simulation for.
Returns: DataFrame (df) containing raw system events.

postprocessing(df, event):
Purpose: Processes the raw system events from the simulation to extract relevant metrics.
Arguments:
df: DataFrame containing raw system events.
event: Name or identifier of the event/scenario.
Returns: DataFrame (data_df) with aggregated metrics.

main():
Purpose: Main function to run the simulation and extract metrics for multiple scenarios and iterations.
Behavior: For each scenario and iteration, it runs the simulation, saves the raw events to a JSON file, then processes the events to extract metrics and saves them to an Excel file.

Execution:
$ python run.py

Control:
To change the number of events or mc runs edit the variables on the following lines
165 starting_event
166 ending_event
167 number_of_mc

5.2 Merging (merger.py)
This script merges multiple Excel files with simulation results into a single aggregated Excel file.

Functions:
main():
Purpose: Main function to merge multiple Excel files.
Behavior: For each scenario, it reads the Excel files of multiple iterations, aggregates the data, and saves the aggregated data to a new Excel file.

Execution:
$ python merger.py

All of the mergers will be saved into runs_merged directory

Control:
To change the number of events or mc runs edit the variables on the following lines
7 starting_event
8 ending_event
9 number_of_mc

5.3 Decoding jsons (json_decoder.py)
This script reads JSON files with simulation results, processes the results, and saves them to Excel files.

Functions:

list_check(item):
Purpose: Checks if an item is a list. If so, returns the first element; otherwise, returns the item itself.
Arguments:
item: The item to check.
Returns: First element of the list (if item is a list) or the item itself.

postprocessing(df, event):
Purpose: Similar to the postprocessing function in run.py, but adapted to handle data from JSON files.
Arguments:
df: DataFrame containing raw system events.
event: Name or identifier of the event/scenario.
Returns: DataFrame (data_df) with aggregated metrics.

main():
Purpose: Main function to read JSON files, process the data, and save it to Excel files.
Behavior: For each iteration, it reads the JSON file, processes the data, and saves the metrics to an Excel file.

Execution:
$ python json_decoder.py

All of the decodings will be saved into runs directory
