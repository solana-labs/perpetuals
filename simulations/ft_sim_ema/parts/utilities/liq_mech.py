import random
from .utils import *
import copy
import numpy as np

def liquidity_provider_decision(liquidity_provider, pool_yield, asset_prices, asset_volatility):
    assets = pool_yield.keys()
    decision = {asset: 0 for asset in assets}
    liquidity_provider = copy.deepcopy(liquidity_provider)

    for asset in assets:
        add_threshold = liquidity_provider['add_threshold'][asset]
        remove_threshold = liquidity_provider['remove_threshold'][asset]

        # Adjust thresholds based on volatility
        if asset_volatility[asset] is not None:
            add_threshold += asset_volatility[asset]
            remove_threshold += asset_volatility[asset]

        asset_yield = pool_yield[asset]

        if asset_yield > add_threshold:
            # The provider adds liquidity proportional to the excess yield
            liquidity_to_add = (liquidity_provider['funds'][asset] / asset_prices[asset][0]) * 10 * (asset_yield - add_threshold)
            decision[asset] += liquidity_to_add
            #print(f"attempt to add {liquidity_to_add} {asset}")

        elif asset_yield < remove_threshold:
            # The provider removes liquidity proportional to the shortfall
            liquidity_to_remove = liquidity_provider['liquidity'][asset] * 10 * (remove_threshold - asset_yield)
            decision[asset] -= liquidity_to_remove
            #print(f"attempt to remove {liquidity_to_remove} {asset}")

    
    return decision

def liquidity_fee(pool, asset, provider_decision, asset_prices, base_fee, ratio_mult):
    pool = copy.deepcopy(pool)
    # if token ratio is improved:           
    #     fee = base_fee / ratio_fee
    # otherwise:         
    #     fee = base_fee * ratio_fee
    # where:          
    # if new_ratio < ratios.target:
    #         ratio_fee = 1 + custody.fees.ratio_mult * (ratios.target - new_ratio) / (ratios.target - ratios.min);          
    # otherwise:
    #     ratio_fee = 1 + custody.fees.ratio_mult * (new_ratio - ratios.target) / (ratios.max - ratios.target);

    target_ratio = pool['target_ratios'][asset]
    tvl = pool_total_holdings(pool, asset_prices)
    current_ratio = (pool['holdings'][asset] * float(asset_prices[asset][0])) / tvl

    new_holding = pool['holdings'][asset] + provider_decision[asset]
    new_tvl = tvl + provider_decision[asset] * float(asset_prices[asset][0])
    new_ratio = new_holding * float(asset_prices[asset][0]) / new_tvl

    if new_ratio - target_ratio > pool['deviation']:
        return -1

    if (new_ratio - current_ratio) < (target_ratio - current_ratio):
        ratio_fee = base_fee / ratio_mult
    else:
        if new_ratio < target_ratio:
            ratio_fee = (1 + ratio_mult * (target_ratio - new_ratio) / (pool['deviation']))/100
        else:
            ratio_fee = (1 + ratio_mult * (new_ratio - target_ratio) / (pool['deviation']))/100

        #ratio_fee = base_fee * ratio_fee

    return ratio_fee

def provide_liquidity(pool, provider, gen_lp, lot_size, asset, provider_pnl, fee, asset_prices):
    tmp_pool = copy.deepcopy(pool)
    tmp_provider = copy.deepcopy(provider)
    tmp_gen = copy.deepcopy(gen_lp)

    if lot_size > 0:
        # check if provider has enough liquidity in funds
        if tmp_provider['funds'][asset] < lot_size + fee:
            return -1
        # # calculate the amount of lp tokens allocated to provider v1
        # tvl = pool_tvl_max(tmp_pool['holdings'], asset_prices)
        # adding_price = asset_prices[asset][0] if asset_prices[asset][0] < asset_prices[asset][1] else asset_prices[asset][1]
        # pool_size_change_lot = lot_size * adding_price / tvl
        # pool_size_change_fee = fee * adding_price / tvl
        # lp_tokens_lot = pool_size_change_lot * tmp_pool['lp_shares']
        # lp_tokens_fee = pool_size_change_fee * tmp_pool['lp_shares']
        # # update provider's liquidity
        # tmp_provider['funds'][asset] -= (lot_size + fee)
        # tmp_provider['liquidity'][asset] += lot_size
        # tmp_provider['pool_share'] += lp_tokens_lot
        # # update genesis provider's liquidity
        # tmp_gen['liquidity'][asset] += fee
        # tmp_gen['pool_share'] += lp_tokens_fee
        # # to holdings add the lot and collected fee v1
        # tmp_pool['total_fees_collected'][asset] += fee
        # tmp_pool['holdings'][asset] += lot_size + fee
        # tmp_pool['lps']["genesis"][asset] += fee
        # tmp_pool['lp_shares'] += lp_tokens_lot + lp_tokens_fee

        # calculate the amount of lp tokens allocated to provider v2
        tvl = pool_tvl_max(tmp_pool['holdings'], asset_prices)
        adding_price = asset_prices[asset][0] if asset_prices[asset][0] < asset_prices[asset][1] else asset_prices[asset][1]
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
        # # calculate the amount of lp tokens allocated to provider v1
        # tvl = pool_tvl_min(tmp_pool['holdings'], asset_prices)
        # removing_price = asset_prices[asset][0] if asset_prices[asset][0] > asset_prices[asset][1] else asset_prices[asset][1]
        # pool_size_change_lot = lot_size * removing_price / tvl
        # pool_size_change_fee = fee * removing_price / tvl
        # lp_tokens_lot = pool_size_change_lot * tmp_pool['lp_shares']
        # lp_tokens_fee = pool_size_change_fee * tmp_pool['lp_shares']

        # calculate the amount of lp tokens allocated to provider
        tvl = pool_tvl_min(tmp_pool['holdings'], asset_prices)
        removing_price = asset_prices[asset][0] if asset_prices[asset][0] > asset_prices[asset][1] else asset_prices[asset][1]
        pool_size_change_lot = lot_size * removing_price / tvl
        lp_tokens_lot = pool_size_change_lot * tmp_pool['lp_shares']
        # check if provider has enough liquidity in funds
        if tmp_provider['id'] in tmp_pool['lps'] and asset in tmp_pool['lps'][tmp_provider['id']] and abs(lp_tokens_lot) <= tmp_provider['pool_share'] and abs(lot_size) + fee <= tmp_provider['liquidity'][asset]:
            # # update provider's liquidity v1
            # tmp_provider['funds'][asset] += abs(lot_size) + provider_pnl - fee
            # tmp_provider['pool_share'] -= (abs(lp_tokens_lot) + abs(lp_tokens_fee))
            # tmp_provider['liquidity'][asset] -= (abs(lot_size) - provider_pnl)
            # # update genesis provider's liquidity
            # tmp_gen['liquidity'][asset] += fee
            # tmp_gen['pool_share'] += lp_tokens_fee
            # # update pool holdings, lps and lp shares
            # tmp_pool['total_fees_collected'][asset] += fee
            # tmp_pool['holdings'][asset] += lot_size + fee - provider_pnl
            # tmp_pool['lps']["genesis"][asset] += fee
            # tmp_pool['lp_shares'] += lp_tokens_fee - lp_tokens_lot
            # tmp_pool['lps'][tmp_provider['id']][asset] += lot_size

            # update provider's liquidity 
            tmp_provider['funds'][asset] += abs(lot_size) + provider_pnl - fee
            tmp_provider['pool_share'] -= abs(lp_tokens_lot)
            tmp_provider['liquidity'][asset] -= (abs(lot_size) - provider_pnl)
            # update genesis provider's liquidity
            tmp_gen['funds'][asset] += fee
            # update pool holdings, lps and lp shares
            tmp_pool['total_fees_collected'][asset] += fee
            tmp_pool['holdings'][asset] += lot_size - provider_pnl
            tmp_pool['lp_shares'] -= lp_tokens_lot
            tmp_pool['lps'][tmp_provider['id']][asset] += lot_size

            return [tmp_pool, tmp_provider, tmp_gen]
        else:
            return -1   
    else:
        return -1
