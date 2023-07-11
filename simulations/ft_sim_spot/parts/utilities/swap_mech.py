import random
from .utils import *
import copy
import numpy as np

def swap_decision(trader_passed, asset, asset_prices, swap_chance):
    trader = copy.deepcopy(trader_passed)

    swap_action = random.random() # 1/5 buy, 1/5 sell, 3/5 do nothing
    asset_held = trader['liquidity'][asset]
    if swap_action < swap_chance[0]: # buy
        swap_in = np.random.uniform(low=0.01, high=0.99) * asset_held
        swap_out_asset = random.choice(list(asset_prices.keys()))

        swap_in_price = asset_prices[asset][0]
        swap_out_price = asset_prices[swap_out_asset][0]

        swap_out = swap_in * swap_in_price / swap_out_price
        return {'swap_in': [swap_in, asset], 'swap_out': [swap_out, swap_out_asset]}
    elif swap_action > swap_chance[1]: # sell
        swap_out = np.random.uniform(low=0.01, high=0.99) * asset_held
        swap_in_asset = random.choice(list(asset_prices.keys()))

        swap_in_price = asset_prices[swap_in_asset][0]
        swap_out_price = asset_prices[asset][0]

        swap_in = swap_out * swap_out_price / swap_in_price
        return {'swap_in': [swap_in, swap_in_asset], 'swap_out': [swap_out, asset]}
    else:
        return None

def swap_fee_calc(pool, token_in, token_in_amt, token_out, token_out_amt, base_fees, om_fees, asset_prices):
    '''
    final fee = pool receiving swap fee + pool paying swap fee + pool receiving base fee + pool paying base fee

    base fees:
    btc: 0.00025
    eth: 0.00025
    sol: 0.00015
    usdc/usdt: 0.0001

    for pool receiving tokens (allocation % up)

    fee = A * (post trade ratio * 100 - target ratio * 100)^3 + fee optimal
    where A = (fee max - fee optional) / (max ratio * 100 - target ratio * 100) ^ 3

    for pool paying tokens (allocation % down)

    fee = A * (post trade ratio * 100 - target ratio * 100)^3 + fee optimal
    where A = (fee max - fee optional) / (min ratio * 100 - target ratio * 100) ^ 3
    
    '''
    pool = copy.deepcopy(pool)
    # return ratio_fee
    tvl = pool_total_holdings(pool, asset_prices)
    fee_max = om_fees[0]
    fee_optimal = om_fees[1]

    target_ratio_in = pool['target_ratios'][token_in]
    post_trade_ratio_in = (pool['holdings'][token_in] + token_in_amt) * float(asset_prices[token_in][0]) / tvl
    max_ratio_in = target_ratio_in + pool['deviation']

    # Calculate the pool receiving swap fee
    A_receiving = (fee_max - fee_optimal) / (max_ratio_in * 100 - target_ratio_in * 100) ** 3
    receiving_fee = A_receiving * (post_trade_ratio_in * 100 - target_ratio_in * 100) ** 3 + fee_optimal

    target_ratio_out = pool['target_ratios'][token_out]
    post_trade_ratio_out = (pool['holdings'][token_out] - token_out_amt) * float(asset_prices[token_out][0]) / tvl
    min_ratio_out = target_ratio_out - pool['deviation']

    # Calculate the pool paying swap fee
    A_paying = (fee_max - fee_optimal) / (min_ratio_out * 100 - target_ratio_out * 100) ** 3
    paying_fee = A_paying * (post_trade_ratio_out * 100 - target_ratio_out * 100) ** 3 + fee_optimal

    # Get the pool receiving base fee and the pool paying base fee
    receiving_base_fee = base_fees[token_in]
    paying_base_fee = base_fees[token_out]

    return [receiving_fee + receiving_base_fee, paying_fee + paying_base_fee]

def swap_tokens_trader(trader_passed, token_in, token_in_amt, token_out, token_out_amt, swap_fee):
    trader = copy.deepcopy(trader_passed)

    trader['liquidity'][token_in] += token_in_amt - swap_fee[1]
    trader['liquidity'][token_out] -= token_out_amt - swap_fee[0]

    if trader['liquidity'][token_in] < 0 or trader['liquidity'][token_out] < 0:
        return -1

    return trader

def swap_tokens_pool(pool, token_in, token_in_amt, token_out, token_out_amt, swap_fee, asset_prices):

    pool = copy.deepcopy(pool)
    # print('swap', pool['lp_shares'], lp_tokens)
    if check_for_avail(pool, token_in, token_in_amt) == -1 or check_for_avail(pool, token_out, token_out_amt) == -1:
        return -1

    pool['holdings'][token_in] -= (token_in_amt - swap_fee[1])
    pool['holdings'][token_out] += token_out_amt + swap_fee[0]
    pool['volume'][token_in] += token_in_amt
    pool['volume'][token_out] += token_out_amt
    pool['total_fees_collected'][token_in] += swap_fee[1]
    pool['total_fees_collected'][token_out] += swap_fee[0]

    tvl = pool_total_holdings(pool, asset_prices)

    post_ratio_in = pool['holdings'][token_in] * asset_prices[token_in][0] / tvl
    post_ratio_out = pool['holdings'][token_out] * asset_prices[token_out][0] / tvl

    if pool['target_ratios'][token_in] - pool['deviation'] < post_ratio_in < pool['target_ratios'][token_in] + pool['deviation'] and pool['target_ratios'][token_out] - pool['deviation'] < post_ratio_out < pool['target_ratios'][token_out] + pool['deviation']:
        return pool
    else:
        return -1
    
def update_gen_lp_swap(tmp_gen_lp, fee, asset):
    updated_gen_lp = copy.deepcopy(tmp_gen_lp)

    lot_size = fee * 0.3
    updated_gen_lp['funds'][asset] += lot_size

    return updated_gen_lp

def swap_tokens(pool, trader, gen_lp, token_in, token_in_amt, token_out, token_out_amt, swap_fee, asset_prices):
    tmp_pool = copy.deepcopy(pool)
    tmp_trader = copy.deepcopy(trader)
    tmp_gen_lp = copy.deepcopy(gen_lp)

    updated_trader = swap_tokens_trader(tmp_trader, token_in, token_in_amt, token_out, token_out_amt, swap_fee)
    if updated_trader != -1:
        updated_pool = swap_tokens_pool(tmp_pool, token_in, token_in_amt, token_out, token_out_amt, swap_fee, asset_prices)
        if updated_pool != -1:
            updated_gen_lp = update_gen_lp_swap(tmp_gen_lp, swap_fee[0], token_out)
            updated_gen_lp = update_gen_lp_swap(updated_gen_lp, swap_fee[1], token_in)
            return updated_pool, updated_trader, updated_gen_lp
        
    return None


