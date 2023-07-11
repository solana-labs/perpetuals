import random
from .utils import *
import copy
import numpy as np

def liquidity_provider_decision(liquidity_provider, pool_yield, asset_prices, asset_volatility):
    assets = pool_yield.keys()
    decision = {asset: 0 for asset in assets}
    lp = liquidity_provider# copy.deepcopy(liquidity_provider)

    for asset in assets:
        add_threshold = lp['add_threshold'][asset]
        remove_threshold = lp['remove_threshold'][asset]

        # Adjust thresholds based on volatility
        if asset_volatility[asset] is not None:
            add_threshold += asset_volatility[asset]
            remove_threshold += asset_volatility[asset]

        asset_yield = pool_yield[asset]

        if asset_yield > add_threshold:
            # The provider adds liquidity proportional to the excess yield
            liquidity_to_add = (lp['funds'][asset] / asset_prices[asset][0]) * 10 * (asset_yield - add_threshold)
            decision[asset] += liquidity_to_add
            #print(f"attempt to add {liquidity_to_add} {asset}")

        elif asset_yield < remove_threshold:
            # The provider removes liquidity proportional to the shortfall
            liquidity_to_remove = lp['liquidity'][asset] * 10 * (remove_threshold - asset_yield)
            decision[asset] -= liquidity_to_remove
            #print(f"attempt to remove {liquidity_to_remove} {asset}")

    return decision

def liquidity_fee(pool_init, asset, provider_decision, asset_prices, base_fees, om_fees):
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
    pool = copy.deepcopy(pool_init)
    # return ratio_fee
    tvl = pool_total_holdings(pool, asset_prices)
    fee_max = om_fees[0]
    fee_optimal = om_fees[1]
    amount = provider_decision[asset]
    # handle spread case
    if amount < 0 or asset.startswith('U'):
        price = asset_prices[asset][0]
    else:
        price = asset_prices[asset][1]

    target_ratio = pool['target_ratios'][asset]
    post_lp_ratio = (pool['holdings'][asset] + amount) * float(price) / tvl
    max_ratio = target_ratio + pool['deviation'] if amount > 0 else target_ratio - pool['deviation']

    # Calculate the pool receiving swap fee
    A = (fee_max - fee_optimal) / (max_ratio * 100 - target_ratio * 100) ** 3
    lp_fee = A * (post_lp_ratio * 100 - target_ratio * 100) ** 3 + fee_optimal

    # Get the pool receiving base fee and the pool paying base fee
    tot_fee = base_fees[asset] + lp_fee

    return tot_fee

def provide_liquidity(pool, provider, gen_lp, lot_size, asset, fee, asset_prices):
    tmp_pool = copy.deepcopy(pool)
    tmp_provider = copy.deepcopy(provider)
    tmp_gen = copy.deepcopy(gen_lp)

    if check_for_avail(pool, asset, abs(lot_size)) == -1:
        return -1

    if lot_size > 0:
        # check if provider has enough liquidity in funds
        if tmp_provider['funds'][asset] < lot_size + fee:
            return -1
        # calculate the amount of lp tokens allocated to provider v2
        tvl = pool_tvl(tmp_pool['holdings'], asset_prices, minmax=1)
        if asset.startswith('U'):
            adding_price = asset_prices[asset][0]
        else:
            adding_price = asset_prices[asset][1]
        pool_size_change_lot = lot_size * adding_price / tvl
        lp_tokens_lot = pool_size_change_lot * tmp_pool['lp_shares']
        # update provider's liquidity
        tmp_provider['funds'][asset] -= (lot_size + fee)
        tmp_provider['liquidity'][asset] += lot_size
        tmp_provider['pool_share'] += lp_tokens_lot
        # update genesis provider's liquidity
        tmp_gen['funds'][asset] += fee
        # to holdings add the lot and collected fee v1
        tmp_pool['total_fees_collected'][asset] += fee
        tmp_pool['holdings'][asset] += lot_size
        tmp_pool['lp_shares'] += lp_tokens_lot

        if tmp_provider['id'] in tmp_pool['lps']:
            if asset in tmp_pool['lps'][tmp_provider['id']]:
                tmp_pool['lps'][tmp_provider['id']][asset] += lot_size
            else:
                tmp_pool['lps'][tmp_provider['id']][asset] = lot_size
        else:
            tmp_pool['lps'][tmp_provider['id']] = {asset: lot_size}
        # print(tmp_pool['lps']['genesis'])
        return [tmp_pool, tmp_provider, tmp_gen]
    
    elif lot_size < 0:
        # calculate the amount of lp tokens allocated to provider
        tvl = pool_tvl(tmp_pool['holdings'], asset_prices, minmax=0)
        removing_price = asset_prices[asset][0]
        pool_size_change_lot = lot_size * removing_price / tvl
        lp_tokens_lot = pool_size_change_lot * tmp_pool['lp_shares']
        # check if provider has enough liquidity in funds
        if tmp_provider['id'] in tmp_pool['lps'] and asset in tmp_pool['lps'][tmp_provider['id']] and abs(lp_tokens_lot) <= tmp_provider['pool_share'] and abs(lot_size) + fee <= tmp_provider['liquidity'][asset]:
            # update provider's liquidity 
            tmp_provider['funds'][asset] += abs(lot_size) - fee
            tmp_provider['pool_share'] -= abs(lp_tokens_lot)
            tmp_provider['liquidity'][asset] -= abs(lot_size)
            # update genesis provider's liquidity
            tmp_gen['funds'][asset] += fee
            # update pool holdings, lps and lp shares
            tmp_pool['total_fees_collected'][asset] += fee
            tmp_pool['holdings'][asset] += (lot_size)
            # if asset == 'SOL':
            #     print(f"removing {lot_size} with pnl {provider_pnl}")
            tmp_pool['lp_shares'] -= lp_tokens_lot
            tmp_pool['lps'][tmp_provider['id']][asset] += lot_size

            return [tmp_pool, tmp_provider, tmp_gen]
        else:
            return -1   
    else:
        return -1