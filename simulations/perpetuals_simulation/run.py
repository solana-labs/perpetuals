import pandas as pd
from parts.utilities.utils import * 
from cadCAD.engine import ExecutionMode, ExecutionContext,Executor
from config import experiments
import json

from sys_params import initial_conditions, sys_params

def run(event):
    '''
    Definition:
    Run simulation
    '''
    try:
        tst_price = fetch_asset_prices(['BTC', 'ETH', 'SOL', 'USDC', 'USDT'], initial_conditions[event]['num_of_min'], sys_params[event]['event'][0])
    except:
        raise ValueError("Number of hours is out of range")
    
    # Single
    exec_mode = ExecutionMode()
    local_mode_ctx = ExecutionContext(context=exec_mode.local_mode)

    simulation = Executor(exec_context=local_mode_ctx, configs=experiments[event].configs)
    raw_system_events, tensor_field, sessions = simulation.execute()
    # Result System Events DataFrame
    df = pd.DataFrame(raw_system_events)
 
    return df

def postprocessing(df, event):
    
    # Initialize an empty list to store each timestep data
    data = []
    # Loop through each row in the dataframe
    for _, row in df.iterrows():
        traders = row['traders']
        liquidity_providers = row['liquidity_providers']
        pools = row['pools']
        liquidations = row['liquidations']
        num_of_longs = row['num_of_longs']
        num_of_shorts = row['num_of_shorts']
        num_of_swaps = row['num_of_swaps']

        nominal_exposure_btc = 0
        nominal_exposure_eth = 0
        nominal_exposure_sol = 0
        for trader in traders.values():
            if 'BTC' in trader['positions_long'] and 'BTC' in trader['positions_short']:
                nominal_exposure_btc += trader['positions_long']['BTC']['quantity'] - trader['positions_short']['BTC']['quantity']
            elif 'BTC' in trader['positions_long']: 
                nominal_exposure_btc += trader['positions_long']['BTC']['quantity']
            elif 'BTC' in trader['positions_short']:
                nominal_exposure_btc -= trader['positions_short']['BTC']['quantity']
            if 'ETH' in trader['positions_long'] and 'ETH' in trader['positions_short']:
                nominal_exposure_eth += trader['positions_long']['ETH']['quantity'] - trader['positions_short']['ETH']['quantity']
            elif 'ETH' in trader['positions_long']:
                nominal_exposure_eth += trader['positions_long']['ETH']['quantity']
            elif 'ETH' in trader['positions_short']:
                nominal_exposure_eth -= trader['positions_short']['ETH']['quantity']
            if 'SOL' in trader['positions_long'] and 'SOL' in trader['positions_short']:
                nominal_exposure_sol += trader['positions_long']['SOL']['quantity'] - trader['positions_short']['SOL']['quantity']
            elif 'SOL' in trader['positions_long']:
                nominal_exposure_sol += trader['positions_long']['SOL']['quantity']
            elif 'SOL' in trader['positions_short']:
                nominal_exposure_sol -= trader['positions_short']['SOL']['quantity']

        # print('asst prices num', i)
        asst_prices = fetch_asset_prices(['BTC', 'ETH', 'SOL', 'USDC', 'USDT'], row['timestep'], event)
        # Generate data for each row
        timestep_data = {
            'timestep': row['timestep'],
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
            'contract_oi_btc_long': pools[0]['contract_oi']['BTC']['oi_long'],
            'contract_oi_btc_short': pools[0]['contract_oi']['BTC']['oi_short'],
            'contract_oi_eth_long': pools[0]['contract_oi']['ETH']['oi_long'],
            'contract_oi_eth_short': pools[0]['contract_oi']['ETH']['oi_short'],
            'contract_oi_sol_long': pools[0]['contract_oi']['SOL']['oi_long'],
            'contract_oi_sol_short': pools[0]['contract_oi']['SOL']['oi_short'],
            'contract_oi_btc_weighted_price_long': pools[0]['contract_oi']['BTC']['weighted_price_long'],
            'contract_oi_btc_weighted_price_short': pools[0]['contract_oi']['BTC']['weighted_price_short'],
            'contract_oi_eth_weighted_price_long': pools[0]['contract_oi']['ETH']['weighted_price_long'],
            'contract_oi_eth_weighted_price_short': pools[0]['contract_oi']['ETH']['weighted_price_short'],
            'contract_oi_sol_weighted_price_long': pools[0]['contract_oi']['SOL']['weighted_price_long'],
            'contract_oi_sol_weighted_price_short': pools[0]['contract_oi']['SOL']['weighted_price_short'],
            'contract_oi_btc_collateral': pools[0]['contract_oi']['BTC']['tot_collateral'],
            'contract_oi_eth_collateral': pools[0]['contract_oi']['ETH']['tot_collateral'],
            'contract_oi_sol_collateral': pools[0]['contract_oi']['SOL']['tot_collateral'],
            'contract_oi_btc_weighted_collateral_price': pools[0]['contract_oi']['BTC']['weighted_collateral_price'],
            'contract_oi_eth_weighted_collateral_price': pools[0]['contract_oi']['ETH']['weighted_collateral_price'],
            'contract_oi_sol_weighted_collateral_price': pools[0]['contract_oi']['SOL']['weighted_collateral_price'],
            'btc_price': asst_prices['BTC'][0],
            'btc_time': str(asst_prices['BTC'][3]),
            'eth_price': asst_prices['ETH'][0],
            'eth_time': str(asst_prices['ETH'][3]),
            'sol_price': asst_prices['SOL'][0],
            'sol_time': str(asst_prices['SOL'][3]),
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
    starting_event = 0
    ending_event = 8
    number_of_mc = 10
    starting_mc = 1
    for i in range(starting_event, ending_event):
        if i == 0:
            starting_mc = 2
        else:
            starting_mc = 1
        for j in range(starting_mc, number_of_mc + 1):
            df = run(i)
            json_data = df.to_json(orient='records', indent=4)
            with open(os.path.join('runs', f'event_{sys_params[i]["event"][0]}_mc{j}.json'), 'w') as file:
                file.write(json_data)
            df = postprocessing(df, sys_params[i]["event"][0])
            exclude_cols = ['btc_time', 'eth_time', 'sol_time']
            df_exclude = df[exclude_cols + ['timestep']]
            df_aggregate = df.drop(columns=exclude_cols)
            agg_df = df_aggregate.groupby('timestep').mean().reset_index()
            df_exclude = df_exclude.groupby('timestep').first().reset_index()
            result_df = pd.merge(agg_df, df_exclude, on='timestep', how='left')
            result_df = result_df.drop(result_df.columns[0], axis=1)
            to_xslx(result_df, os.path.join('runs', f'event_{sys_params[i]["event"][0]}_mc{j}')) 


if __name__ == '__main__':
    main()