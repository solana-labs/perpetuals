import pandas as pd
from parts.utilities.utils import * 
from cadCAD.engine import ExecutionMode, ExecutionContext,Executor
from config import exp
import json

from sys_params import initial_conditions, sys_params


def run():
    '''
    Definition:
    Run simulation
    '''
    try:
        tst_price = fetch_asset_prices(['BTC', 'ETH', 'SOL', 'USDC', 'USDT'], initial_conditions['num_of_hrs'], sys_params['event'][0], sys_params['start_date'][0])
    except:
        raise ValueError("Number of hours is out of range")

    # Single
    exec_mode = ExecutionMode()
    local_mode_ctx = ExecutionContext(context=exec_mode.local_mode)

    simulation = Executor(exec_context=local_mode_ctx, configs=exp.configs)
    raw_system_events, tensor_field, sessions = simulation.execute()
    # Result System Events DataFrame
    df = pd.DataFrame(raw_system_events)
 
    return df

def postprocessing(df):
    json_data = df.to_json(orient='records', indent=4)

    with open('data.json', 'w') as file:
        file.write(json_data)
    
    # Pool balances and ratios
    # Amount of traders and providers
    # Cumulative PnL for traders, max and min PnL for traders (per market)
    # Cumulative apy for liquidity providers (per market)
    # Open interest (long and short supplies per market)
    # Volumes
    # Number of liquidations
    # Fees collected
    # Treasury balance

    # track pnl of the pool as metric
    # Initialize an empty list to store each timestep data
    data = []
    # Loop through each row in the dataframe
    for _, row in df.iterrows():
        traders = row['traders']
        liquidity_providers = row['liquidity_providers']
        pools = row['pools']
        # treasury = row['treasury']
        liquidations = row['liquidations']
        num_of_longs = row['num_of_longs']
        num_of_shorts = row['num_of_shorts']
        num_of_swaps = row['num_of_swaps']

        nominal_exposure_btc = 0
        nominal_exposure_eth = 0
        nominal_exposure_sol = 0
        for trader in traders.values():
            # print(trader)
            if 'BTC' in trader['positions_long'] and 'BTC' in trader['positions_short']:
                # print(f"nom exp btc {trader['positions_long']['BTC']['quantity']} + {trader['positions_short']['BTC']['quantity']}")
                nominal_exposure_btc += trader['positions_long']['BTC']['quantity'] - trader['positions_short']['BTC']['quantity']
            elif 'BTC' in trader['positions_long']: 
                nominal_exposure_btc += trader['positions_long']['BTC']['quantity']
            elif 'BTC' in trader['positions_short']:
                nominal_exposure_btc -= trader['positions_short']['BTC']['quantity']
            if 'ETH' in trader['positions_long'] and 'ETH' in trader['positions_short']:
                # print(f"nom exp eth {trader['positions_long']['ETH']['quantity']} + {trader['positions_short']['ETH']['quantity']}")
                nominal_exposure_eth += trader['positions_long']['ETH']['quantity'] - trader['positions_short']['ETH']['quantity']
            elif 'ETH' in trader['positions_long']:
                nominal_exposure_eth += trader['positions_long']['ETH']['quantity']
            elif 'ETH' in trader['positions_short']:
                nominal_exposure_eth -= trader['positions_short']['ETH']['quantity']
            if 'SOL' in trader['positions_long'] and 'SOL' in trader['positions_short']:
                # print(f"nom exp sol {trader['positions_long']['SOL']['quantity']} + {trader['positions_short']['SOL']['quantity']}")
                nominal_exposure_sol += trader['positions_long']['SOL']['quantity'] - trader['positions_short']['SOL']['quantity']
            elif 'SOL' in trader['positions_long']:
                nominal_exposure_sol += trader['positions_long']['SOL']['quantity']
            elif 'SOL' in trader['positions_short']:
                nominal_exposure_sol -= trader['positions_short']['SOL']['quantity']

        # Generate data for each row
        timestep_data = {
            'number_of_traders': len(traders),
            'number_of_liquidity_providers': len(liquidity_providers),
            'pool_lp_tokens': pools[0]['lp_shares'],
            'pool_balance_btc': pools[0]['holdings']['BTC'],
            'pool_balance_eth': pools[0]['holdings']['ETH'],
            'pool_balance_sol': pools[0]['holdings']['SOL'],
            'pool_balance_usdc': pools[0]['holdings']['USDC'],
            'pool_balance_usdt': pools[0]['holdings']['USDT'],
            'cum_pnl_traders': sum(trader['PnL'] for trader in traders.values()),
            'max_pnl_traders': max(trader['PnL'] for trader in traders.values()),
            'min_pnl_traders': min(trader['PnL'] for trader in traders.values()),
            # 'cum_apy_providers': sum(lp['yield'] for lp in liquidity_providers.values()),  # Assuming each LP has a 'yield' key
            'oi_long_btc': pools[0]['oi_long']['BTC'],
            'oi_long_eth': pools[0]['oi_long']['ETH'],
            'oi_long_sol': pools[0]['oi_long']['SOL'],
            'oi_short_btc': pools[0]['oi_short']['BTC'],
            'oi_short_eth': pools[0]['oi_short']['ETH'],
            'oi_short_sol': pools[0]['oi_short']['SOL'],
            'volume_btc': pools[0]['volume']['BTC'],
            'volume_eth': pools[0]['volume']['ETH'],
            'volume_sol': pools[0]['volume']['SOL'],
            'num_of_longs': num_of_longs,
            'num_of_shorts': num_of_shorts,
            'num_of_swaps': num_of_swaps,
            'number_of_liquidations': liquidations,
            'fees_collected_btc': pools[0]['total_fees_collected']['BTC'],
            'fees_collected_eth': pools[0]['total_fees_collected']['ETH'],
            'fees_collected_sol': pools[0]['total_fees_collected']['SOL'],
            'fees_collected_usdc': pools[0]['total_fees_collected']['USDC'],
            'fees_collected_usdt': pools[0]['total_fees_collected']['USDT'],
            'treasury_balance_btc': liquidity_providers['genesis']['liquidity']['BTC'] + liquidity_providers['genesis']['funds']['BTC'],
            'treasury_balance_eth': liquidity_providers['genesis']['liquidity']['ETH'] + liquidity_providers['genesis']['funds']['ETH'],
            'treasury_balance_sol': liquidity_providers['genesis']['liquidity']['SOL'] + liquidity_providers['genesis']['funds']['SOL'],
            'treasury_balance_usdc': liquidity_providers['genesis']['liquidity']['USDC'] + liquidity_providers['genesis']['funds']['USDC'],
            'treasury_balance_usdt': liquidity_providers['genesis']['liquidity']['USDT'] + liquidity_providers['genesis']['funds']['USDT'],
            'pool_open_pnl_btc': pools[0]['open_pnl_long']['BTC'] + pools[0]['open_pnl_short']['BTC'], 
            'pool_open_pnl_eth': pools[0]['open_pnl_long']['ETH'] + pools[0]['open_pnl_short']['ETH'],
            'pool_open_pnl_sol': pools[0]['open_pnl_long']['SOL'] + pools[0]['open_pnl_short']['SOL'],
            'nominal_exposure_btc': nominal_exposure_btc,
            'nominal_exposure_eth': nominal_exposure_eth,
            'nominal_exposure_sol': nominal_exposure_sol,
            'pool_tvl': pools[0]['tvl'],
            'pool_perc_btc': pools[0]['pool_ratios']['BTC'],
            'pool_perc_eth': pools[0]['pool_ratios']['ETH'],
            'pool_perc_sol': pools[0]['pool_ratios']['SOL'],
            'pool_perc_usdc': pools[0]['pool_ratios']['USDC'],
            'pool_perc_usdt': pools[0]['pool_ratios']['USDT'],
            'lp_bal_btc': sum(lp['liquidity']['BTC'] for lp in liquidity_providers.values()) - liquidity_providers['genesis']['liquidity']['BTC'],
            'lp_bal_eth': sum(lp['liquidity']['ETH'] for lp in liquidity_providers.values()) - liquidity_providers['genesis']['liquidity']['ETH'],
            'lp_bal_sol': sum(lp['liquidity']['SOL'] for lp in liquidity_providers.values()) - liquidity_providers['genesis']['liquidity']['SOL'],
            'lp_bal_usdc': sum(lp['liquidity']['USDC'] for lp in liquidity_providers.values()) - liquidity_providers['genesis']['liquidity']['USDC'],
            'lp_bal_usdt': sum(lp['liquidity']['USDT'] for lp in liquidity_providers.values()) - liquidity_providers['genesis']['liquidity']['USDT'],
            'short_interest_usdc': pools[0]['short_interest']['USDC'],
            'short_interest_usdt': pools[0]['short_interest']['USDT'],
            'short_interest_tot': pools[0]['short_interest']['USDC'] + pools[0]['short_interest']['USDT'],
        }
        
        # Append the timestep data to the list
        data.append(timestep_data)

    # Convert the list of timestep data into a DataFrame
    data_df = pd.DataFrame(data)

    return data_df

def main():
    '''
    Definition:
    Run simulation and extract metrics
    '''
    df = run()
    df = postprocessing(df)
    to_xslx(df, f'run_{sys_params["event"][0]}') 
    df = df[::3].reset_index(drop=True)
    to_xslx(df, f'run_merged_{sys_params["event"][0]}') 
    return df

if __name__ == '__main__':
    main()