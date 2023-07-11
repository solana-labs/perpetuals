from .utilities.utils import *
from .utilities.liq_mech import *
import copy
import time

def liquidity_policy(params, substep, state_history, previous_state):
    liquidity_providers = copy.deepcopy(previous_state['liquidity_providers'])
    pools = copy.deepcopy(previous_state['pools'])
    timestep = previous_state['timestep']
    print(timestep, 'liquidity')

    p = 0
    for pool_id in pools.keys():
        pool = pools[pool_id]
        gen_prov = copy.deepcopy(liquidity_providers['genesis'])
        asset_volatility = get_asset_volatility(pool['assets'], timestep, params['event'])
        asset_prices = fetch_asset_prices(pool['assets'], timestep, params['event'])

        tvl = pool_total_holdings(pool, asset_prices)
        for asset in pool['assets']:
            pool['tvl'] = tvl
            pool['pool_ratios'][asset] = pool['holdings'][asset] * asset_prices[asset][0] / tvl

        # print(pool['yield'])
        for liquidity_provider_id in liquidity_providers.keys():

            liquidity_provider = liquidity_providers[liquidity_provider_id]

            # synch the provider liquidity with the pool holdings value based on their pool share
            if liquidity_provider_id in pool['lps']:
                provider_balance = get_provider_balance(liquidity_provider, asset_prices)
                # print('prov bal', provider_balance)
                updated_liquidity = {'BTC': 0, 'SOL': 0, 'ETH': 0, 'USDC': 0, 'USDT': 0}
                for asset in pool['lps'][liquidity_provider_id].keys():
                    liq_ratio = liquidity_provider['liquidity'][asset] * asset_prices[asset][0] / provider_balance
                    # print('---------', pool['holdings'][asset], asset, liq_ratio)
                    updated_liquidity[asset] = pool['holdings'][asset] * liq_ratio * liquidity_provider['pool_share'] / (pool['lp_shares'] * pool['pool_ratios'][asset])
                    # print('+++++++++', pool['holdings'][asset], asset, updated_liquidity[asset], liquidity_provider['pool_share'] / pool['lp_shares'])

                    pool['lps'][liquidity_provider_id][asset] = updated_liquidity[asset]

                # print(updated_liquidity)
                liquidity_provider['liquidity'] = updated_liquidity.copy()

            if liquidity_provider_id == 'genesis':
                continue
            # print("ass vol",asset_volatility)
            provider_decision = liquidity_provider_decision(liquidity_provider, pool['yield'], asset_prices, asset_volatility)

            for asset in provider_decision.keys():
                if provider_decision[asset] == 0:
                    continue
                
                # if problem don't allow adding liquidity
                if not asset.startswith("U") and asset_prices[asset][2] == True and provider_decision[asset] > 0:
                    continue

                #provder_open_pnl = (liquidity_provider['liquidity'][asset] / pool['holdings'][asset]) * (pool['open_pnl_long'][asset] + pool['open_pnl_short'][asset])
                if provider_decision[asset] < 0:
                    # check provider in the pool and change his decision to withdraw all liquidity (assumption that if someone wants out they withdraw all
                    provider_id = liquidity_provider['id']
                    if provider_id in pool['lps']:
                        if asset in pool['lps'][provider_id]:
                            provider_decision[asset] = -1 * (pool['lps'][provider_id][asset]) #+ provder_open_pnl)
                        else:
                            continue
                    else:
                        continue

                # consider the open pnl of the pool in proportion to the provider
                lot_size = provider_decision[asset]

                # Fetch the fee amount
                fee_perc = liquidity_fee(pool, asset, provider_decision, asset_prices, params['base_fees_swap'], params['om_fees_swap'])
                # fee amount returns -1 if the provider decision if does not pass the constraints
                if fee_perc == -1:
                    continue
                # calculate the fee
                fee_amount = abs(lot_size * fee_perc)
                if fee_amount / lot_size > 0.07:
                    continue
                # update the provider and pool values
                #res_tmp = provide_liquidity(pool, liquidity_provider, gen_prov, lot_size, asset, provder_open_pnl, fee_amount, asset_prices)
                res_tmp = provide_liquidity(pool, liquidity_provider, gen_prov, lot_size, asset, fee_amount, asset_prices)

                if res_tmp == -1:
                    continue
                liquidity_provider = res_tmp[1]
                gen_prov = res_tmp[2]
                pool = res_tmp[0]
                # if timestep == 10:
                #     pool['yield'] = {'BTC': 0.001, 'SOL': 0.001, 'ETH': 0.001, 'USDC': 0.001, 'USDT': 0.001}
            liquidity_providers[liquidity_provider_id] = liquidity_provider
        liquidity_providers['genesis'] = gen_prov
        pools[pool_id] = pool
        p += 1
        
    action = {
        'liquidity_providers': liquidity_providers,
        'pools': pools,
    }

    return action

def liquidity_providers_update(params, substep, state_history, previous_state, policy):
    key = 'liquidity_providers'
    value = policy['liquidity_providers']
    return (key, value)

def pool_liquidity_update(params, substep, state_history, previous_state, policy):
    key = 'pools'
    value = policy['pools']
    return (key, value)