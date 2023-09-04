import random
from .utils import *
import copy
import numpy as np

def liquidity_provider_decision(liquidity_provider, pool_yield, asset_prices, asset_volatility, ratios):
    assets = pool_yield.keys()
    decision = {asset: 0 for asset in assets}
    lp = liquidity_provider

    asset_yield = 0
    volat = 0
    for ast in pool_yield.keys():
        asset_yield += pool_yield[ast] * ratios[ast]
        # Adjust thresholds based on volatility
        if asset_volatility[ast] is not None:
            volat += asset_volatility[ast] * 3

    for asset in assets:
        add_threshold = lp['add_threshold'][asset]
        remove_threshold = lp['remove_threshold'][asset]

        add_threshold += volat
        remove_threshold += volat

        # change to the general pool yield and fees dependency
        if asset_yield > add_threshold:
            if lp['funds'][asset] * asset_prices[asset][0] < 3:
                continue
            # The provider adds liquidity proportional to the excess yield
            liquidity_to_add = lp['funds'][asset] * 10 * (asset_yield - add_threshold)
            if liquidity_to_add > lp['funds'][asset]:
                liquidity_to_add = 0.95 * lp['funds'][asset]
            decision[asset] += liquidity_to_add

        elif asset_yield < remove_threshold:
            if lp['liquidity'][asset] * asset_prices[asset][0] < 3:
                continue
            # The provider removes liquidity proportional to the shortfall
            liquidity_to_remove = lp['liquidity'][asset] * 40 * (remove_threshold - asset_yield)
            if liquidity_to_remove > lp['liquidity'][asset]:
                liquidity_to_remove = 0.95 * lp['liquidity'][asset]
            decision[asset] -= liquidity_to_remove

    return decision

def liquidity_fee(pool_init, asset, provider_decision, asset_prices, fees):
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
    #'lp_fees': [{'add_base_fee': 0.005, 'optimal_fee': 0.001, 'max_fee': 0.025, 'rm_base_fee': 0.0005}]

    pool = copy.deepcopy(pool_init)
    tvl = pool_total_holdings(pool, asset_prices)
    fee_max = fees['max_fee']
    fee_optimal = fees['optimal_fee']
    amount = provider_decision[asset]
    # handle spread case
    if amount < 0 or asset.startswith('U'):
        price = asset_prices[asset][0]
    else:
        price = asset_prices[asset][1]

    target_ratio = pool['target_ratios'][asset]
    post_lp_ratio = (pool['holdings'][asset] + amount) * float(price) / tvl
    max_ratio = pool['max_ratio'][asset]
    min_ratio = pool['min_ratio'][asset]

    if amount > 0:
        if post_lp_ratio > max_ratio:
            return -1
        elif post_lp_ratio > target_ratio:
            hslope = (fee_max - fee_optimal) / (max_ratio - target_ratio)
            hb = fee_optimal - target_ratio * hslope
            lp_fee = hslope * post_lp_ratio + hb
        else:
            lslope = (fee_max - fee_optimal) / (target_ratio - min_ratio)
            lb = fee_optimal - target_ratio * lslope
            lp_fee = lslope * post_lp_ratio + lb
        
        return lp_fee + fees['add_base_fee']

    elif amount < 0:
        if post_lp_ratio < min_ratio:
            return -1
        elif post_lp_ratio < target_ratio:
            hslope = -(fee_max - fee_optimal) / (max_ratio - target_ratio)
            hb = fee_optimal - target_ratio * hslope
            lp_fee = hslope * post_lp_ratio + hb
        else:
            lslope = -(fee_max - fee_optimal) / (target_ratio - min_ratio)
            lb = fee_optimal - target_ratio * lslope
            lp_fee = lslope * post_lp_ratio + lb

        return lp_fee + fees['add_base_fee'] + fees['rm_base_fee']

    else:
        return -1

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
            tmp_pool['lp_shares'] -= lp_tokens_lot
            tmp_pool['lps'][tmp_provider['id']][asset] += lot_size

            return [tmp_pool, tmp_provider, tmp_gen]
        else:
            return -1   
    else:
        return -1