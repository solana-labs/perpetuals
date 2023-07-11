
import pandas as pd
import numpy as np
import copy
import os
import random

year = 'HR'

def fetch_asset_prices(assets, timestep):
    asset_prices = {}
    for asset in assets:
        try:
            file_path = os.path.join('data', f'{asset}{year}.xlsx')
            df = pd.read_excel(file_path)
        except:
            file_path = os.path.join('parts', 'data', f'{asset}{year}.xlsx')
            df = pd.read_excel(file_path)
        price_high = df.loc[timestep, 'High']
        price_low = df.loc[timestep, 'Low']
        price_ema = df.loc[timestep, 'ema']
        asset_prices.update({asset: [price_low, price_high, price_ema]})
    return asset_prices

def get_asset_prices(asset_prices):
    asset_prices = copy.deepcopy(asset_prices)
    for asset in asset_prices.keys():
        #print(asset_prices)
        asset_prices[asset] = [asset_prices[asset][0] + random.random() * (asset_prices[asset][1] - asset_prices[asset][0]), asset_prices[asset][2]]
    return asset_prices

def get_asset_volatility(assets, timestep):
    asset_volatility = {}
    for asset in assets:
        try:
            file_path = os.path.join('data', f'{asset}{year}.xlsx')
            df = pd.read_excel(file_path)
        except:
            file_path = os.path.join('parts', 'data', f'{asset}{year}.xlsx')
            df = pd.read_excel(file_path)    
        try:
            close_prices = df.loc[timestep-10:timestep, 'Close']
            price_std = close_prices.std()
            price_avg = close_prices.mean()
            volatility = price_std / price_avg  # Volatility represented as average % standard deviation
        except:
            volatility = None  # If there isn't enough data to compute volatility, return None

        asset_volatility.update({asset: volatility})
    return asset_volatility

def pool_total_holdings(pool, asset_prices):
    holdings = pool['holdings']
    tvl = 0
    for asset in holdings.keys():
        tvl += holdings[asset] * asset_prices[asset][0]
    return tvl

def pool_tvl(holdings, asset_prices):
    tvl = 0
    for asset in holdings.keys():
        tvl += holdings[asset] * asset_prices[asset][0]
    return tvl

def pool_tvl_max(holdings, asset_prices):
    tvl = 0
    for asset in holdings.keys():
        tvl += holdings[asset] * asset_prices[asset][0] if asset_prices[asset][0] > asset_prices[asset][1] else holdings[asset] * asset_prices[asset][1]
    return tvl

def pool_tvl_min(holdings, asset_prices):
    tvl = 0
    for asset in holdings.keys():
        tvl += holdings[asset] * asset_prices[asset][0] if asset_prices[asset][0] < asset_prices[asset][1] else holdings[asset] * asset_prices[asset][1]
    return tvl

def get_account_value(trader, asset_prices):
    total_value = sum([trader['liquidity'][asset] * asset_prices[asset][0] for asset in trader['liquidity'].keys()])
    total_value += sum([(trader['positions'][asset][0] * asset_prices[asset][0] - trader['loans'][asset][0]) for asset in trader['positions'].keys()])
    return total_value

def calculate_interest(position_size, duration, asset, pool, rate_params):
    if year == 'HR':
        duration = duration/24
    optimal_utilization = rate_params[0]
    slope1 = rate_params[1]
    slope2 = rate_params[2]

    total_holdings = pool['holdings'][asset]
    total_borrowed = pool['oi_long'][asset] + pool['oi_short'][asset]

    # Handle division by zero
    if total_holdings == 0:
        return 0

    current_utilization = total_borrowed / total_holdings

    if current_utilization < optimal_utilization:
        rate = (current_utilization / optimal_utilization) * slope1
    else:
        rate = slope1 + (current_utilization - optimal_utilization) / (1 - optimal_utilization) * slope2

    cumulative_interest = duration * rate
    borrow_fee_amount = cumulative_interest * position_size

    return borrow_fee_amount


def calculate_open_pnl(traders, asset_prices):
    open_pnl_long = {asset: 0 for asset in asset_prices.keys()}
    open_pnl_short = {asset: 0 for asset in asset_prices.keys()}
    for trader_id in traders.keys():
        for asset in traders[trader_id]['positions_long'].keys():
            open_pnl_long[asset] -= traders[trader_id]['positions_long'][asset]['quantity'] * (asset_prices[asset][0] - traders[trader_id]['positions_long'][asset]['entry_price'])
        for asset in traders[trader_id]['positions_short'].keys():
            open_pnl_short[asset] -= traders[trader_id]['positions_short'][asset]['quantity'] * (asset_prices[asset][0] - traders[trader_id]['positions_short'][asset]['entry_price'])

    return [open_pnl_long, open_pnl_short]