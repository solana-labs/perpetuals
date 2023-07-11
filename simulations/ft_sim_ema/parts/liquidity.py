from .utilities.utils import *
from .utilities.liq_mech import *
import copy

def liquidity_policy(params, substep, state_history, previous_state):
    liquidity_providers = copy.deepcopy(previous_state['liquidity_providers'])
    pools = copy.deepcopy(previous_state['pools'])
    fees_collected = []
    timestep = previous_state['timestep']
    print(timestep, 'liquidity')

    p = 0
    for pool_id in pools.keys():
        pool = pools[pool_id]
        gen_prov = copy.deepcopy(liquidity_providers['genesis'])
        asset_volatility = get_asset_volatility(pool['assets'], timestep)
        fees_collected.append({ast: 0 for ast in pool['assets']})
        price_dict = fetch_asset_prices(pool['assets'], timestep)

        # print(pool['yield'])

        for liquidity_provider_id in liquidity_providers.keys():
            if liquidity_provider_id == 'genesis':
                continue
            liquidity_provider = liquidity_providers[liquidity_provider_id]
            asset_prices = get_asset_prices(price_dict)
            # print("ass vol",asset_volatility)
            provider_decision = liquidity_provider_decision(liquidity_provider, pool['yield'], asset_prices, asset_volatility)

            for asset in provider_decision.keys():
                if provider_decision[asset] == 0:
                    continue

                provder_open_pnl = (liquidity_provider['liquidity'][asset] / pool['holdings'][asset]) * (pool['open_pnl_long'][asset] + pool['open_pnl_short'][asset])

                if provider_decision[asset] < 0:
                    # check provider in the pool and change his decision to withdraw all liquidity (assumption that if someone wants out they withdraw all
                    provider_id = liquidity_provider['id']
                    if provider_id in pool['lps']:
                        if asset in pool['lps'][provider_id]:
                            provider_decision[asset] = -1 * (pool['lps'][provider_id][asset] + provder_open_pnl)
                        else:
                            continue
                    else:
                        continue

                # consider the open pnl of the pool in proportion to the provider
                lot_size = provider_decision[asset]

                # Fetch the fee amount
                fee_perc = liquidity_fee(pool, asset, provider_decision, asset_prices, params['base_fee'], params['ratio_mult'])
                # fee amount returns -1 if the provider decision if does not pass the constraints
                if fee_perc == -1:
                    continue
                # calculate the fee
                fee_amount = abs(lot_size * fee_perc)
                if fee_amount / lot_size > 0.07:
                    continue
                # update the provider and pool values
                res_tmp = provide_liquidity(pool, liquidity_provider, gen_prov, lot_size, asset, provder_open_pnl, fee_amount, asset_prices)
                if res_tmp == -1:
                    continue
                liquidity_provider = res_tmp[1]
                gen_prov = res_tmp[2]
                pool = res_tmp[0]
                # if timestep == 10:
                #     pool['yield'] = {'BTC': 0.001, 'SOL': 0.001, 'ETH': 0.001, 'USDC': 0.001, 'USDT': 0.001}
                fees_collected[p][asset] += fee_amount
            liquidity_providers[liquidity_provider_id] = liquidity_provider
        liquidity_providers['genesis'] = gen_prov
        pools[pool_id] = pool
        p += 1
        
    action = {
        'liquidity_providers': liquidity_providers,
        'pools': pools,
        'fees_collected': fees_collected
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

def treasury_liquidity_update(params, substep, state_history, previous_state, policy):
    key = 'treasury'
    value = {asset: previous_state['treasury'][asset] + sum([policy['fees_collected'][i][asset] for i in range(len(policy['fees_collected']))]) for asset in previous_state['treasury'].keys()}
    return (key, value)