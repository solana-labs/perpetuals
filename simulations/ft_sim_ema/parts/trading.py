from .utilities.utils import *
from .utilities.trad_mech import *
from .utilities.swap_mech import *

def trading_policy(params, substep, state_history, previous_state):

    traders = copy.deepcopy(previous_state['traders'])
    pools = copy.deepcopy(previous_state['pools'])
    liquidity_providers = copy.deepcopy(previous_state['liquidity_providers'])
    timestep = previous_state['timestep']
    liquidations = previous_state['liquidations']
    num_of_longs = 0
    num_of_shorts = 0
    num_of_swaps = 0
    gen_lp = copy.deepcopy(liquidity_providers['genesis'])

    print(timestep, 'traders')

    p = 0
    for pool_id in pools.keys():
        pool = pools[pool_id]
        price_dict = fetch_asset_prices(pool['assets'], timestep)

        for trader_id in traders.keys():
            trader = traders[trader_id]
            asset_prices = get_asset_prices(price_dict)

            for asset in pool['assets']:
                # print(f"this is trade {asset} {trader['liquidity'][asset]}")
                if asset == 'USDT' or asset == 'USDC':
                    continue

                trade_decision = trading_decision(trader, timestep, asset, asset_prices[asset], params['max_margin'], params['liquidation_threshold'], pool, params['rate_params'], params['trade_chance'])

                if trade_decision['long'] == None and trade_decision['short'] == None:
                    # print('no trade')
                    continue

                # print('trade decision', trade_decision)

                if trade_decision['short'] != None and trade_decision['short']['direction'] == 'open':
                    if trade_decision['short']['swap'] != 0:
                        tokens_in = trade_decision['short']['denomination']
                        tokens_out = 'USDT' if tokens_in == 'USDC' else 'USDC'
                        swap_fee = swap_fee_calc(pool, tokens_in, trade_decision['short']['swap'], tokens_out, trade_decision['short']['swap'], params['base_fees_swap'], params['om_fees_swap'], asset_prices)

                        swap_res = swap_tokens(pool, trader, gen_lp, tokens_in, trade_decision['short']['swap'], tokens_out, trade_decision['short']['swap'], swap_fee, asset_prices)
                        if swap_res != None:
                            pool, trader, gen_lp = swap_res
                            num_of_swaps += 1

                # Fetch the fee amount
                fees = trading_fee(pool, asset, trade_decision, params['rate_params'], params['ratio_mult'])

                exec_long = execute_long(pool, trader, gen_lp, trade_decision, fees, asset, timestep, asset_prices)
                if exec_long != None:
                    exec_short = execute_short(exec_long[0], exec_long[1], exec_long[2], trade_decision, fees, asset, timestep, asset_prices)
                    if exec_short != None:
                        pool, trader, gen_lp = exec_short
                    else:
                        pool, trader, gen_lp = exec_long
                else:
                    exec_short = execute_short(pool, trader, gen_lp, trade_decision, fees, asset, timestep, asset_prices)
                    if exec_short != None:
                        pool, trader, gen_lp = exec_short
                    else:
                        continue

                if exec_long != None:
                    # print('longed')
                    num_of_longs += 1
                if exec_short != None:
                    # print('shorted')
                    num_of_shorts += 1
            
            asset_prices = get_asset_prices(price_dict)
            for asset in pool['assets']:
                # print(f"this is swap {asset} {trader['liquidity'][asset]}")

                swaping_decision = swap_decision(trader, asset, asset_prices, params['swap_chance'])

                if swaping_decision == None:
                    continue

                swap_in = swaping_decision['swap_in']
                swap_out = swaping_decision['swap_out']
                swap_fee = swap_fee_calc(pool, swap_in[1], swap_in[0], swap_out[1], swap_out[0], params['base_fees_swap'], params['om_fees_swap'], asset_prices)

                swap_res = swap_tokens(pool, trader, gen_lp, swap_in[1], swap_in[0], swap_out[1], swap_out[0], swap_fee, asset_prices)

                if swap_res == None:
                    continue

                pool, trader, gen_lp = swap_res
                num_of_swaps += 1

            traders[trader_id] = trader
            liquidity_providers['genesis'] = gen_lp

        # update the pool values
        # update pool open pnl
        open_pnl = calculate_open_pnl(traders, asset_prices)
        pool['open_pnl_long'] = open_pnl[0]
        pool['open_pnl_short'] = open_pnl[1]

        total_provider_fees_collected = {}
        # update yield and lp fees
        for asset in pool['assets']:
            total_provider_fees_collected[asset] = (pool['total_fees_collected'][asset] - previous_state['pools'][pool_id]['total_fees_collected'][asset]) * 0.7
            pool['yield'][asset] = 0.7 * (365*24) * (pool['total_fees_collected'][asset] - previous_state['pools'][pool_id]['total_fees_collected'][asset]) / pool['holdings'][asset]
        
        # calculate amount of lp tokens
        for provider_id in pool['lps'].keys():
            for asset in pool['lps'][provider_id].keys():
                # calculate number of lp shares
                provider_share = total_provider_fees_collected[asset] * (liquidity_providers[provider_id]['liquidity'][asset] / pool['holdings'][asset])
                if provider_id == 'genesis':
                    provider_share = total_provider_fees_collected[asset] * ((liquidity_providers[provider_id]['liquidity'][asset] - total_provider_fees_collected[asset] * 0.3 / 0.7) / pool['holdings'][asset])

                # update provider
                liquidity_providers[provider_id]['funds'][asset] += provider_share


        # # calculate amount of lp tokens
        # for provider_id in pool['lps'].keys():
        #     for asset in pool['lps'][provider_id].keys():
        #         # calculate number of lp shares
        #         tvl = pool_tvl_max(pool['holdings'], asset_prices)
        #         provider_share = total_provider_fees_collected[asset] * (liquidity_providers[provider_id]['liquidity'][asset] / pool['holdings'][asset])
        #         if provider_id == 'genesis':
        #             provider_share = total_provider_fees_collected[asset] * ((liquidity_providers[provider_id]['liquidity'][asset] - total_provider_fees_collected[asset] * 0.3 / 0.7) / pool['holdings'][asset])
        #         adding_price = asset_prices[asset][0] if asset_prices[asset][0] < asset_prices[asset][1] else asset_prices[asset][1]
        #         pool_size_change = provider_share * adding_price / tvl
        #         lp_tokens = pool_size_change * pool['lp_shares']

        #         # update provider
        #         liquidity_providers[provider_id]['liquidity'][asset] += provider_share
        #         liquidity_providers[provider_id]['pool_share'] += lp_tokens

        #         # update pool
        #         pool['lps'][provider_id][asset] += provider_share
        #         pool['lp_shares'] += lp_tokens
        #         pool['holdings'][asset] += provider_share

        pools[pool_id] = pool
        p += 1
        
    action = {
        'traders': traders,
        'pools': pools,
        'liquidity_providers': liquidity_providers,
        'liquidations': liquidations,
        'num_of_longs': num_of_longs,
        'num_of_shorts': num_of_shorts,
        'num_of_swaps': num_of_swaps,
        'gen_lp': gen_lp
    }

    return action

def traders_update(params, substep, state_history, previous_state, policy):
    key = 'traders'
    value = policy['traders']
    return (key, value)

def pool_trading_update(params, substep, state_history, previous_state, policy):
    key = 'pools'
    value = policy['pools']
    return (key, value)

def liquidations_uodate(params, substep, state_history, previous_state, policy):
    key = 'liquidations'
    value = policy['liquidations']
    return (key, value)

def distribution_providers_update(params, substep, state_history, previous_state, policy):
    key = 'liquidity_providers'
    value = policy['liquidity_providers']
    return (key, value)

def num_of_longs_update(params, substep, state_history, previous_state, policy):
    key = 'num_of_longs'
    value = previous_state['num_of_longs'] + policy['num_of_longs']
    return (key, value)

def num_of_shorts_update(params, substep, state_history, previous_state, policy):
    key = 'num_of_shorts'
    value = previous_state['num_of_shorts'] + policy['num_of_shorts']
    return (key, value)

def num_of_swaps_update(params, substep, state_history, previous_state, policy):
    key = 'num_of_swaps'
    value = previous_state['num_of_swaps'] + policy['num_of_swaps']
    return (key, value)

# def oracle_attack_update(params, substep, state_history, previous_state, policy):
#     key = 'oracle_attack'
#     value = previous_state['oracle_attack']
#     return (key, value)