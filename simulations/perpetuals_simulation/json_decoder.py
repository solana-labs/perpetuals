import pandas as pd
import json
from parts.utilities.utils import * 
from sys_params import initial_conditions, sys_params
import openpyxl


def list_check(item):
    if isinstance(item, list):
        return item[0]
    else:
        return item

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
                nominal_exposure_btc += list_check(trader['positions_long']['BTC']['quantity']) - list_check(trader['positions_short']['BTC']['quantity'])
            elif 'BTC' in trader['positions_long']: 
                nominal_exposure_btc += list_check(trader['positions_long']['BTC']['quantity'])
            elif 'BTC' in trader['positions_short']:
                nominal_exposure_btc -= list_check(trader['positions_short']['BTC']['quantity'])
            if 'ETH' in trader['positions_long'] and 'ETH' in trader['positions_short']:
                nominal_exposure_eth += list_check(trader['positions_long']['ETH']['quantity']) - list_check(trader['positions_short']['ETH']['quantity'])
            elif 'ETH' in trader['positions_long']:
                nominal_exposure_eth += list_check(trader['positions_long']['ETH']['quantity'])
            elif 'ETH' in trader['positions_short']:
                nominal_exposure_eth -= list_check(trader['positions_short']['ETH']['quantity'])
            if 'SOL' in trader['positions_long'] and 'SOL' in trader['positions_short']:
                nominal_exposure_sol += list_check(trader['positions_long']['SOL']['quantity']) - list_check(trader['positions_short']['SOL']['quantity'])
            elif 'SOL' in trader['positions_long']:
                nominal_exposure_sol += list_check(trader['positions_long']['SOL']['quantity'])
            elif 'SOL' in trader['positions_short']:
                nominal_exposure_sol -= list_check(trader['positions_short']['SOL']['quantity'])

        asst_prices = fetch_asset_prices(['BTC', 'ETH', 'SOL', 'USDC', 'USDT'], row['timestep'], event)

        # Generate data for each row
        timestep_data = {
            'timestep': row['timestep'],
            'number_of_traders': len(traders),
            'number_of_liquidity_providers': len(liquidity_providers),
            'pool_lp_tokens': list_check(pools['0']['lp_shares']),
            'pool_balance_btc': list_check(pools['0']['holdings']['BTC']),
            'pool_balance_eth': list_check(pools['0']['holdings']['ETH']),
            'pool_balance_sol': list_check(pools['0']['holdings']['SOL']),
            'pool_balance_usdc': list_check(pools['0']['holdings']['USDC']),
            'pool_balance_usdt': list_check(pools['0']['holdings']['USDT']),
            'cum_pnl_traders': sum(list_check(trader['PnL']) for trader in traders.values()),
            'max_pnl_traders': max(list_check(trader['PnL']) for trader in traders.values()),
            'min_pnl_traders': min(list_check(trader['PnL']) for trader in traders.values()),
            'oi_long_btc': list_check(pools['0']['oi_long']['BTC']),
            'oi_long_eth': list_check(pools['0']['oi_long']['ETH']),
            'oi_long_sol': list_check(pools['0']['oi_long']['SOL']),
            'oi_short_btc': list_check(pools['0']['oi_short']['BTC']),
            'oi_short_eth': list_check(pools['0']['oi_short']['ETH']),
            'oi_short_sol': list_check(pools['0']['oi_short']['SOL']),
            'volume_btc': list_check(pools['0']['volume']['BTC']),
            'volume_eth': list_check(pools['0']['volume']['ETH']),
            'volume_sol': list_check(pools['0']['volume']['SOL']),
            'num_of_longs': num_of_longs,
            'num_of_shorts': num_of_shorts,
            'num_of_swaps': num_of_swaps,
            'number_of_liquidations': liquidations,
            'fees_collected_btc': list_check(pools['0']['total_fees_collected']['BTC']),
            'fees_collected_eth': list_check(pools['0']['total_fees_collected']['ETH']),
            'fees_collected_sol': list_check(pools['0']['total_fees_collected']['SOL']),
            'fees_collected_usdc': list_check(pools['0']['total_fees_collected']['USDC']),
            'fees_collected_usdt': list_check(pools['0']['total_fees_collected']['USDT']),
            'treasury_balance_btc': list_check(liquidity_providers['genesis']['liquidity']['BTC']) + list_check(liquidity_providers['genesis']['funds']['BTC']),
            'treasury_balance_eth': list_check(liquidity_providers['genesis']['liquidity']['ETH']) + list_check(liquidity_providers['genesis']['funds']['ETH']),
            'treasury_balance_sol': list_check(liquidity_providers['genesis']['liquidity']['SOL']) + list_check(liquidity_providers['genesis']['funds']['SOL']),
            'treasury_balance_usdc': list_check(liquidity_providers['genesis']['liquidity']['USDC']) + list_check(liquidity_providers['genesis']['funds']['USDC']),
            'treasury_balance_usdt': list_check(liquidity_providers['genesis']['liquidity']['USDT']) + list_check(liquidity_providers['genesis']['funds']['USDT']),
            'pool_open_pnl_btc': list_check(pools['0']['open_pnl_long']['BTC']) + list_check(pools['0']['open_pnl_short']['BTC']), 
            'pool_open_pnl_eth': list_check(pools['0']['open_pnl_long']['ETH']) + list_check(pools['0']['open_pnl_short']['ETH']),
            'pool_open_pnl_sol': list_check(pools['0']['open_pnl_long']['SOL']) + list_check(pools['0']['open_pnl_short']['SOL']),
            'nominal_exposure_btc': nominal_exposure_btc,
            'nominal_exposure_eth': nominal_exposure_eth,
            'nominal_exposure_sol': nominal_exposure_sol,
            'pool_tvl': list_check(pools['0']['tvl']),
            'pool_perc_btc': list_check(pools['0']['pool_ratios']['BTC']),
            'pool_perc_eth': list_check(pools['0']['pool_ratios']['ETH']),
            'pool_perc_sol': list_check(pools['0']['pool_ratios']['SOL']),
            'pool_perc_usdc': list_check(pools['0']['pool_ratios']['USDC']),
            'pool_perc_usdt': list_check(pools['0']['pool_ratios']['USDT']),
            'lp_bal_btc': sum(list_check(lp['liquidity']['BTC']) for lp in liquidity_providers.values()) - list_check(liquidity_providers['genesis']['liquidity']['BTC']),
            'lp_bal_eth': sum(list_check(lp['liquidity']['ETH']) for lp in liquidity_providers.values()) - list_check(liquidity_providers['genesis']['liquidity']['ETH']),
            'lp_bal_sol': sum(list_check(lp['liquidity']['SOL']) for lp in liquidity_providers.values()) - list_check(liquidity_providers['genesis']['liquidity']['SOL']),
            'lp_bal_usdc': sum(list_check(lp['liquidity']['USDC']) for lp in liquidity_providers.values()) - list_check(liquidity_providers['genesis']['liquidity']['USDC']),
            'lp_bal_usdt': sum(list_check(lp['liquidity']['USDT']) for lp in liquidity_providers.values()) - list_check(liquidity_providers['genesis']['liquidity']['USDT']),
            'short_interest_usdc': list_check(pools['0']['short_interest']['USDC']),
            'short_interest_usdt': list_check(pools['0']['short_interest']['USDT']),
            'short_interest_tot': list_check(pools['0']['short_interest']['USDC']) + list_check(pools['0']['short_interest']['USDT']),
            'contract_oi_btc_long': list_check(pools['0']['contract_oi']['BTC']['oi_long']),
            'contract_oi_btc_short': list_check(pools['0']['contract_oi']['BTC']['oi_short']),
            'contract_oi_eth_long': list_check(pools['0']['contract_oi']['ETH']['oi_long']),
            'contract_oi_eth_short': list_check(pools['0']['contract_oi']['ETH']['oi_short']),
            'contract_oi_sol_long': list_check(pools['0']['contract_oi']['SOL']['oi_long']),
            'contract_oi_sol_short': list_check(pools['0']['contract_oi']['SOL']['oi_short']),
            'contract_oi_btc_weighted_price_long': list_check(pools['0']['contract_oi']['BTC']['weighted_price_long']),
            'contract_oi_btc_weighted_price_short': list_check(pools['0']['contract_oi']['BTC']['weighted_price_short']),
            'contract_oi_eth_weighted_price_long': list_check(pools['0']['contract_oi']['ETH']['weighted_price_long']),
            'contract_oi_eth_weighted_price_short': list_check(pools['0']['contract_oi']['ETH']['weighted_price_short']),
            'contract_oi_sol_weighted_price_long': list_check(pools['0']['contract_oi']['SOL']['weighted_price_long']),
            'contract_oi_sol_weighted_price_short': list_check(pools['0']['contract_oi']['SOL']['weighted_price_short']),
            'contract_oi_btc_collateral': list_check(pools['0']['contract_oi']['BTC']['tot_collateral']),
            'contract_oi_eth_collateral': list_check(pools['0']['contract_oi']['ETH']['tot_collateral']),
            'contract_oi_sol_collateral': list_check(pools['0']['contract_oi']['SOL']['tot_collateral']),
            'contract_oi_btc_weighted_collateral_price': list_check(pools['0']['contract_oi']['BTC']['weighted_collateral_price']),
            'contract_oi_eth_weighted_collateral_price': list_check(pools['0']['contract_oi']['ETH']['weighted_collateral_price']),
            'contract_oi_sol_weighted_collateral_price': list_check(pools['0']['contract_oi']['SOL']['weighted_collateral_price']),
            'btc_price': list_check(asst_prices['BTC'][0]),
            'btc_time': str(asst_prices['BTC'][3]),
            'eth_price': list_check(asst_prices['ETH'][0]),
            'eth_time': str(asst_prices['ETH'][3]),
            'sol_price': list_check(asst_prices['SOL'][0]),
            'sol_time': str(asst_prices['SOL'][3]),
        }
        
        # Append the timestep data to the list
        data.append(timestep_data)

    # Convert the list of timestep data into a DataFrame
    data_df = pd.DataFrame(data)

    return data_df


def main():
    for i in range(1, 11):
        df = pd.read_json(os.path.join('runs', f'event_1_mc{i}.json'))
        # df.to_excel('runs/tst.xlsx')
        df = postprocessing(df, sys_params[0]["event"][0])
        exclude_cols = ['btc_time', 'eth_time', 'sol_time']
        df_exclude = df[exclude_cols + ['timestep']]
        df_aggregate = df.drop(columns=exclude_cols)
        agg_df = df_aggregate.groupby('timestep').mean().reset_index()
        df_exclude = df_exclude.groupby('timestep').first().reset_index()
        result_df = pd.merge(agg_df, df_exclude, on='timestep', how='left')
        result_df = result_df.drop(result_df.columns[0], axis=1)
        to_xslx(result_df, os.path.join('runs', f'event_{sys_params[0]["event"][0]}_mc{i}')) 

if __name__ == "__main__":
    main()
