import random
from .utils import *
import copy
import numpy as np

def trading_fee(pool, asset, trade_decision, rate_params, max_payoff_mult):
    # if new_utilization < custody.borrow_rate.optimal_utilization 
    #     entry_fee = (custody.fees.open_position * position.size)
    # else
    #     entry_fee = custody.fees.open_position * utilization_fee * size

    # where:
    #     utilization_fee = 1 + custody.fees.utilization_mult * (new_utilization - optimal_utilization) / (1 - optimal_utilization);
    #     optimum_utilization = optimum_utilization from custody
    #     new_utilization = custody.assets.locked + (position.size * custody.pricing.max_payoff_mult)
    long_fee = 0
    short_fee = 0
    optimal_utilization = rate_params[0]

    # Handle long
    if trade_decision['long'] != None:
        # Handle open
        if trade_decision['long']['direction'] == 'open':
            new_utilization = pool['oi_long'][asset] + (trade_decision['long']['quantity'] * max_payoff_mult)
            if new_utilization < optimal_utilization:
                long_fee = pool['fees']['open'] * trade_decision['long']['quantity']
            else:
                utilization_fee = (1 + pool['utilization_mult'][asset] * (new_utilization - optimal_utilization) / (1 - optimal_utilization))/100
                long_fee = pool['fees']['open'] * utilization_fee * trade_decision['long']['quantity']
        # Handle close
        elif trade_decision['long']['direction'] == 'close':
            long_fee = pool['fees']['close'] * trade_decision['long']['quantity']

    # Handle short
    if trade_decision['short'] != None:
        # Handle open
        if trade_decision['short']['direction'] == 'open':
            new_utilization = pool['oi_short'][asset] + (trade_decision['short']['quantity'] * max_payoff_mult)
            if new_utilization < optimal_utilization:
                short_fee = pool['fees']['open'] * trade_decision['short']['quantity']
            else:
                utilization_fee = (1 + pool['utilization_mult'][asset] * (new_utilization - optimal_utilization) / (1 - optimal_utilization))/100
                short_fee = pool['fees']['open'] * utilization_fee * trade_decision['short']['quantity']
        # Handle close
        elif trade_decision['short']['direction'] == 'close':
            short_fee = pool['fees']['close'] * trade_decision['short']['quantity']

    return [long_fee, short_fee]

def close_long(trader, timestep, asset, asset_price, liquidated, pool, rate_params):
    trader = copy.deepcopy(trader)

    usd_pnl = (asset_price - trader['positions_long'][asset]['entry_price']) * trader['positions_long'][asset]['quantity']
    pnl = usd_pnl / asset_price
    duration = timestep - trader[f'positions_long'][asset]['timestep']
    interest = calculate_interest(trader[f'positions_long'][asset]['quantity'], duration, asset, pool, rate_params)
    payout = trader[f'positions_long'][asset]['nominal_collateral'] / asset_price - interest + pnl
    collateral_pnl = trader[f'positions_long'][asset]['nominal_collateral'] / asset_price - trader[f'positions_long'][asset]['collateral']

    decision = {
        'quantity': trader['positions_long'][asset]['quantity'],
        'payout': payout,
        'interest_paid': interest,
        'PnL': pnl,
        'usd_pnl': usd_pnl,
        'collateral_pnl': collateral_pnl,
        'collateral': trader[f'positions_long'][asset]['collateral'],
        'liquidation': liquidated,
        'direction': 'close'
    }
    # print("long", payout, trader['liquidity'][asset], trader[f'positions_long'][asset]['collateral'], interest, pnl)

    return decision


def close_short(trader, timestep, asset, asset_price, liquidated, pool, rate_params):
    trader = copy.deepcopy(trader)

    pnl = (trader['positions_short'][asset]['entry_price'] - asset_price) * trader['positions_short'][asset]['quantity']
    duration = timestep - trader[f'positions_short'][asset]['timestep']
    interest = calculate_interest(trader[f'positions_short'][asset]['quantity'], duration, asset, pool, rate_params)
    payout = trader[f'positions_short'][asset]['collateral']['amount'] - interest + pnl

    decision = {
        'quantity': trader['positions_short'][asset]['quantity'],
        'payout': payout,
        'interest_paid': interest,
        'PnL': pnl,
        'liquidation': liquidated,
        'denomination': trader[f'positions_short'][asset]['collateral']['denomination'], # {token: {quantity: 0, entry_price: 0, collateral: {amount: 0, denomination: "USDC"}, timestep: 0}}
        'direction': 'close',
        'asset_price': asset_price
    }
    # print("short", payout, trader['liquidity'][asset], trader[f'positions_short'][asset]['collateral']['amount'], interest, pnl)
    return decision

def trading_decision(trader_passed, timestep, asset, asset_pricing, max_margin, liquidation_threshold, pool, rate_params, trade_chance):
    trader = copy.deepcopy(trader_passed)
    pool = copy.deepcopy(pool)

    cs_price = asset_pricing[0]
    cl_price = asset_pricing[1]
    os_price = asset_pricing[1]
    ol_price = asset_pricing[0]

    decision = {
        'long': None,
        'short': None
    }
    
    # Handle liquidations or position expirations
    if asset in trader['positions_long'] and trader['positions_long'][asset]['quantity'] != 0:
        if timestep - trader['positions_long'][asset]['timestep'] >= trader['avg_position_hold'] * np.random.uniform(low=0.8, high=1.4):
            decision['long'] = close_long(trader, timestep, asset, cl_price, False, pool, rate_params)
            # print(f"long closed {decision['long']['quantity']} of {asset} at {asset_price}")

        usd_pnl = (cl_price - trader['positions_long'][asset]['entry_price']) * trader['positions_long'][asset]['quantity']
        pnl = usd_pnl / cl_price
        duration = timestep - trader[f'positions_long'][asset]['timestep']
        interest = calculate_interest(trader[f'positions_long'][asset]['quantity'], duration, asset, pool, rate_params)
        payout = trader[f'positions_long'][asset]['nominal_collateral'] / cl_price - interest + pnl
        collateral_pnl = trader[f'positions_long'][asset]['nominal_collateral'] / cl_price - trader[f'positions_long'][asset]['collateral']
        # if payout < 0:
        #     print('cl', payout, trader[f'positions_long'][asset]['collateral'], interest, pnl)
        if payout / trader['positions_long'][asset]['quantity'] < liquidation_threshold:
            decision['long'] = {
                'quantity': trader['positions_long'][asset]['quantity'],
                'payout': payout,
                'interest_paid': interest,
                'PnL': pnl,
                'usd_pnl': usd_pnl,
                'collateral_pnl': collateral_pnl,
                'collateral': trader[f'positions_long'][asset]['collateral'],
                'liquidation': True,
                'direction': 'close'
            }

    if asset in trader['positions_short'] and trader['positions_short'][asset]['quantity'] != 0:
        if timestep - trader['positions_short'][asset]['timestep'] >= trader['avg_position_hold'] * np.random.uniform(low=0.8, high=1.4):
            decision['short'] = close_short(trader, timestep, asset, cs_price, False, pool, rate_params)

        pnl = (trader['positions_short'][asset]['entry_price'] - cs_price) * trader['positions_short'][asset]['quantity']
        duration = timestep - trader[f'positions_short'][asset]['timestep']
        interest = calculate_interest(trader[f'positions_short'][asset]['quantity'], duration, asset, pool, rate_params)
        payout = trader[f'positions_short'][asset]['collateral']['amount'] - interest + pnl
        # if payout < 0:
        #     print('cs', payout, trader[f'positions_short'][asset]['collateral']['amount'], interest, pnl)
        if payout / trader['positions_short'][asset]['quantity'] < liquidation_threshold:
            decision['short'] = {
                'quantity': trader['positions_short'][asset]['quantity'],
                'payout': payout,
                'interest_paid': interest,
                'PnL': pnl,
                'liquidation': True,
                'denomination': trader[f'positions_short'][asset]['collateral']['denomination'], # {token: {quantity: 0, entry_price: 0, collateral: {amount: 0, denomination: "USDC"}, timestep: 0}}
                'direction': 'close',
                'asset_price': cs_price
            }
    
    if asset_pricing[2] == True:
        return decision

    trade_action = random.random() # 1/4 enter a long, 1/4 enter a short, 1/2 do nothing. if position was closed then pass
    available_asset = pool['holdings'][asset] - (pool['oi_long'][asset] + pool['oi_short'][asset])

    if timestep < 3:
        trade_chance = [0.4, 0.6]

    if trade_action < trade_chance[0] and decision['long'] == None: # enter a long
        asset_held = trader['liquidity'][asset]
        if asset_held > 0:
            max_leverage_lot = asset_held * max_margin
            lot_size = np.random.uniform(low=0.01, high=(trader['risk_factor'] / 10)) * max_leverage_lot * random.random()
            risk_factor_lot = np.random.uniform(low=0.01, high=(trader['risk_factor'] / 10)) * max_leverage_lot
            cap = 1 if max_leverage_lot < available_asset else available_asset / max_leverage_lot
            lot_size = risk_factor_lot * np.random.uniform(low=0.01, high=cap)
            interest = 0
            # charge interest if position exits
            if asset in trader['positions_long'] and trader['positions_long'][asset]['quantity'] != 0:
                duration = timestep - trader[f'positions_long'][asset]['timestep']
                interest = calculate_interest(trader[f'positions_long'][asset]['quantity'], duration, asset, pool, rate_params)

            required_collateral = lot_size / max_margin
            bot = (required_collateral + interest) / asset_held
            if bot < 1:
                collateral_added = asset_held * np.random.uniform(low=bot, high=1)
                # print(f"longed {lot_size} of {asset} with {collateral_added} of collateral and {collateral_added/lot_size} ratio at {spot_price} and avail asset {available_asset}")
                decision['long'] = {
                    'quantity': lot_size,
                    'asset_price': ol_price,
                    'collateral': collateral_added,
                    'interest_paid': interest,
                    'direction': "open"
                }
                # print(f"long {asset} {lot_size} at {ol_price} asset held {asset_held}")


    elif trade_action > trade_chance[1] and decision['short'] == None: # enter a short
        # print('dec to short')
        usd_liquidity = trader['liquidity']['USDC'] + trader['liquidity']['USDT']
        if usd_liquidity > 0:
            max_leverage_lot = (usd_liquidity / os_price) * max_margin
            risk_factor_lot = np.random.uniform(low=0.01, high=(trader['risk_factor'] / 10)) * max_leverage_lot
            cap = 1 if max_leverage_lot < available_asset else available_asset / max_leverage_lot
            lot_size = risk_factor_lot * np.random.uniform(low=0.01, high=cap) * random.random()
            interest = 0
            # charge interest if position exits
            if asset in trader['positions_short'] and trader['positions_short'][asset]['quantity'] != 0:
                duration = timestep - trader[f'positions_short'][asset]['timestep']
                interest = calculate_interest(trader[f'positions_short'][asset]['quantity'], duration, asset, pool, rate_params)
                denomination  = trader['positions_short'][asset]['collateral']['denomination']
            required_collateral = (lot_size * os_price) / max_margin
            bot = (required_collateral + interest) / usd_liquidity
            if bot < 1:
                collateral_added = usd_liquidity * np.random.uniform(low=bot, high=1)
                swap = 0    
                # Choose the stable to use
                if asset not in trader['positions_short']:
                    if trader['liquidity']['USDC'] > collateral_added and trader['liquidity']['USDT'] > collateral_added:
                        denomination = 'USDC' if random.random() > 0.5 else 'USDT'
                    elif trader['liquidity']['USDC'] > collateral_added:
                        denomination = 'USDC'
                    elif trader['liquidity']['USDT'] > collateral_added:
                        denomination = 'USDT'
                    else:
                        denomination = 'USDC'
                        swap = collateral_added - trader['liquidity']['USDC']

                # print(f"shorted {lot_size} of {asset} with {collateral_added} of collateral and {collateral_added/lot_size} ratio at {asset_price}")
                
                decision['short'] = {
                    'quantity': lot_size,
                    'asset_price': os_price,
                    'collateral': collateral_added,
                    'interest_paid': interest,
                    'denomination': denomination,
                    'swap': swap,
                    'direction': 'open'
                }
                # print(f"short {asset} {lot_size} at {os_price}")

    return decision

def update_trader_open_long(trader, trade_decision, fees, asset, timestep):
    updated_trader = copy.deepcopy(trader)

    # If there's a long decision
    long_asset = trade_decision['long']
    long_fee = fees[0]
    long_quantity = long_asset['quantity']
    long_collateral = long_asset['collateral']
    
    # Deduct the collateral, fee from the liquidity and subtract interest
    updated_trader['liquidity'][asset] -= long_fee
    updated_trader['liquidity'][asset] -= trade_decision['long']['interest_paid']
    updated_trader['liquidity'][asset] -= long_collateral

    # Check if enough liquidity for the transaction
    if updated_trader['liquidity'][asset] < 0:
        return -1

    # Update the positions
    if asset in updated_trader['positions_long']:
        updated_trader['positions_long'][asset]['entry_price'] = (long_asset['asset_price'] * long_quantity + updated_trader['positions_long'][asset]['entry_price'] * updated_trader['positions_long'][asset]['quantity']) / (long_quantity + updated_trader['positions_long'][asset]['quantity'])
        updated_trader['positions_long'][asset]['quantity'] += long_quantity
        updated_trader['positions_long'][asset]['collateral'] += long_collateral
        updated_trader['positions_long'][asset]['nominal_collateral'] += long_collateral * trade_decision['long']['asset_price']
        updated_trader['positions_long'][asset]['timestep'] = timestep
    else:
        updated_trader['positions_long'][asset] = {
            'quantity': long_quantity,
            'entry_price': long_asset['asset_price'],
            'collateral': long_collateral,
            'nominal_collateral': long_collateral * long_asset['asset_price'],
            'timestep': timestep
        }
    return updated_trader

def update_trader_open_short(trader, trade_decision, fees, asset, timestep):
    updated_trader = copy.deepcopy(trader)

    short_asset = trade_decision['short']
    short_fee = fees[1]
    short_quantity = short_asset['quantity']
    short_collateral = short_asset['collateral']
    denomination = short_asset['denomination']

    # Deduct the collateral, fee and interest from the liquidity
    updated_trader['liquidity'][denomination] -= short_fee
    updated_trader['liquidity'][asset] -= trade_decision['short']['interest_paid']
    updated_trader['liquidity'][denomination] -= short_collateral

    # Check if enough liquidity for the transaction
    if updated_trader['liquidity'][denomination] < 0:
        return -1

    # Update the positions
    if asset in updated_trader['positions_short']:
        updated_trader['positions_short'][asset]['entry_price'] = (short_asset['asset_price'] * short_quantity + updated_trader['positions_short'][asset]['entry_price'] * updated_trader['positions_short'][asset]['quantity']) / (short_quantity + updated_trader['positions_short'][asset]['quantity'])
        updated_trader['positions_short'][asset]['quantity'] += short_quantity
        updated_trader['positions_short'][asset]['collateral']['amount'] += short_collateral
        updated_trader['positions_short'][asset]['collateral']['denomination'] = denomination
        updated_trader['positions_short'][asset]['timestep'] = timestep  # {token: {quantity: 0, entry_price: 0, collateral: {amount: 0, denomination: "USDC"}, timestep: 0}}
    else:
        updated_trader['positions_short'][asset] = {
            'quantity': short_quantity,
            'entry_price': short_asset['asset_price'],
            'collateral': {
                'amount': short_collateral,
                'denomination': denomination
            },
            'timestep': timestep
        }
    return updated_trader

def update_pool_open_long(pool, trader, asset, trade_decision, fees):
    updated_pool = copy.deepcopy(pool)

    # Check if the pool has enough space for the trade
    available_asset = updated_pool['holdings'][asset] - updated_pool['oi_long'][asset]# + updated_pool['oi_short'][asset])

    if available_asset < trade_decision['long']['quantity']:
        return -1

    # Increase the open interest
    updated_pool['oi_long'][asset] += trade_decision['long']['quantity']
    updated_cont_oi = trade_decision['long']['quantity'] + updated_pool['contract_oi'][asset]['oi_long']
    updated_tot_collateral = trade_decision['long']['collateral'] + updated_pool['contract_oi'][asset]['tot_collateral']
    updated_pool['contract_oi'][asset]['avg_price_long'] = updated_pool['contract_oi'][asset]['avg_price_long'] * (updated_pool['contract_oi'][asset]['oi_long']/updated_cont_oi) + trade_decision['long']['asset_price'] * (trade_decision['long']['quantity']/updated_cont_oi)
    updated_pool['contract_oi'][asset]['avg_collateral_price'] = updated_pool['contract_oi'][asset]['avg_collateral_price'] * (updated_pool['contract_oi'][asset]['tot_collateral']/updated_tot_collateral) + trade_decision['long']['asset_price'] * (trade_decision['long']['collateral']/updated_tot_collateral)
    updated_pool['contract_oi'][asset]['oi_long'] = updated_cont_oi
    updated_pool['contract_oi'][asset]['tot_collateral'] = updated_tot_collateral
    updated_pool['volume'][asset] += trade_decision['long']['quantity']
    updated_pool['total_fees_collected'][asset] += fees[0] + trade_decision['long']['interest_paid']

    # Update loan book
    if trader['id'] not in updated_pool['loan_book_longs']:
        updated_pool['loan_book_longs'][trader['id']] = {}
    if asset not in updated_pool['loan_book_longs'][trader['id']]:
        updated_pool['loan_book_longs'][trader['id']][asset] = {'amount': trade_decision['long']['quantity'], 'collateral': trade_decision['long']['collateral']}
    else:
        updated_pool['loan_book_longs'][trader['id']][asset]['amount'] += trade_decision['long']['quantity']
        updated_pool['loan_book_longs'][trader['id']][asset]['collateral'] += trade_decision['long']['collateral']

    return updated_pool

def update_pool_open_short(pool, trader, asset, trade_decision, fees):
    updated_pool = copy.deepcopy(pool)

    # Check if the pool has enough space for the trade
    #available_asset = updated_pool['holdings'][asset] - (updated_pool['oi_long'][asset] + updated_pool['oi_short'][asset])
    available_asset = updated_pool['holdings'][trade_decision['short']['denomination']] - updated_pool['short_interest'][trade_decision['short']['denomination']]

    if available_asset < trade_decision['short']['quantity'] * trade_decision['short']['asset_price']:
        return -1
    
    # Increase the open interest
    updated_pool['oi_short'][asset] += trade_decision['short']['quantity']
    updated_cont_oi = trade_decision['short']['quantity'] + updated_pool['contract_oi'][asset]['oi_short']
    updated_pool['contract_oi'][asset]['avg_price_short'] = updated_pool['contract_oi'][asset]['avg_price_short'] * (updated_pool['contract_oi'][asset]['oi_short']/updated_cont_oi) + trade_decision['short']['asset_price'] * (trade_decision['short']['quantity']/updated_cont_oi)
    updated_pool['contract_oi'][asset]['oi_short'] = updated_cont_oi
    updated_pool['short_interest'][trade_decision['short']['denomination']] += trade_decision['short']['quantity'] * trade_decision['short']['asset_price']
    updated_pool['volume'][asset] += trade_decision['short']['quantity']
    updated_pool['total_fees_collected'][trade_decision['short']['denomination']] += fees[1] + trade_decision['short']['interest_paid']

    # Update loan book
    if trader['id'] not in updated_pool['loan_book_shorts']:
        updated_pool['loan_book_shorts'][trader['id']] = {}
    if asset not in updated_pool['loan_book_shorts'][trader['id']]:
        updated_pool['loan_book_shorts'][trader['id']][asset] = {'amount': trade_decision['short']['quantity'], 'collateral': trade_decision['short']['collateral']}
    else:
        updated_pool['loan_book_shorts'][trader['id']][asset]['amount'] += trade_decision['short']['quantity']
        updated_pool['loan_book_shorts'][trader['id']][asset]['collateral'] += trade_decision['short']['collateral']

    return updated_pool

def update_gen_lp(tmp_gen_lp, fee, interest, asset):
    updated_gen_lp = copy.deepcopy(tmp_gen_lp)

    lot_size = (fee + interest) * 0.3

    updated_gen_lp['funds'][asset] += lot_size

    return updated_gen_lp

def update_trader_close_long(trader, trade_decision, asset):

    updated_trader = copy.deepcopy(trader)

    updated_trader['liquidity'][asset] += trade_decision['long']['payout']
    updated_trader['PnL'] += trade_decision['long']['usd_pnl']
    # detete position
    del updated_trader['positions_long'][asset]

    return updated_trader

def update_trader_close_short(trader, trade_decision, asset):

    updated_trader = copy.deepcopy(trader)

    updated_trader['liquidity'][trade_decision['short']['denomination']] += trade_decision['short']['payout']
    updated_trader['PnL'] += trade_decision['short']['PnL']
    # detete position
    del updated_trader['positions_short'][asset]

    return updated_trader

def update_pool_close_long(pool, trader, asset, trade_decision, fees):
    updated_pool = copy.deepcopy(pool)

    # Decrease the open interest
    updated_pool['oi_long'][asset] -= trade_decision['long']['quantity']
    updated_pool['contract_oi'][asset]['oi_long'] -= trade_decision['long']['quantity']
    updated_pool['contract_oi'][asset]['tot_collateral'] -= trade_decision['long']['collateral']
    updated_pool['volume'][asset] += trade_decision['long']['quantity']
    updated_pool['total_fees_collected'][asset] += fees[0] + trade_decision['long']['interest_paid']
    updated_pool['holdings'][asset] -= trade_decision['long']['PnL']
    updated_pool['holdings'][asset] -= trade_decision['long']['collateral_pnl']

    # Update loan book
    if trader['id'] not in updated_pool['loan_book_longs']:
        return -1
    if asset not in updated_pool['loan_book_longs'][trader['id']]:
        return -1
    else:
        del updated_pool['loan_book_longs'][trader['id']][asset]
        if updated_pool['loan_book_longs'][trader['id']] == {}:
            del updated_pool['loan_book_longs'][trader['id']]

    return updated_pool

def update_pool_close_short(pool, trader, asset, trade_decision, fees):
    updated_pool = copy.deepcopy(pool)

    # Decrease the open interest
    updated_pool['oi_short'][asset] -= trade_decision['short']['quantity']
    updated_pool['contract_oi'][asset]['oi_short'] -= trade_decision['short']['quantity']
    updated_pool['short_interest'][trade_decision['short']['denomination']] -= trade_decision['short']['quantity'] * trade_decision['short']['asset_price']
    updated_pool['volume'][asset] += trade_decision['short']['quantity']
    updated_pool['total_fees_collected'][trade_decision['short']['denomination']] += fees[1] + trade_decision['short']['interest_paid']
    updated_pool['holdings'][trade_decision['short']['denomination']] -= trade_decision['short']['PnL']

    # Update loan book
    if trader['id'] not in updated_pool['loan_book_shorts']:
        return -1
    if asset not in updated_pool['loan_book_shorts'][trader['id']]:
        return -1
    else:
        del updated_pool['loan_book_shorts'][trader['id']][asset]
        if updated_pool['loan_book_shorts'][trader['id']] == {}:
            del updated_pool['loan_book_shorts'][trader['id']]
    
    return updated_pool

def execute_long(pool, trader, gen_lp, trade_decision, fees, asset, timestep):
    tmp_pool = copy.deepcopy(pool)
    tmp_trader = copy.deepcopy(trader)
    tmp_gen_lp = copy.deepcopy(gen_lp)

    if trade_decision['long'] != None:
        if trade_decision['long']['direction'] == 'open':
            # Update the trader subtract the liquidity, add position with collateral, if position already exists subtract interest
            updated_trader = update_trader_open_long(tmp_trader, trade_decision, fees, asset, timestep)
            if updated_trader != -1:
                # Update the pool
                updated_pool = update_pool_open_long(tmp_pool, updated_trader, asset, trade_decision, fees)
                if updated_pool != -1:
                    # Update the genesis lp and pool
                    # updated_pool, updated_gen_lp = update_gen_lp(updated_pool, tmp_gen_lp, fees[0], trade_decision['long']['interest_paid'], asset, asset_prices)
                    updated_gen_lp = update_gen_lp(tmp_gen_lp, fees[0], trade_decision['long']['interest_paid'], asset)
                    tmp_pool = updated_pool
                    tmp_trader = updated_trader
                    tmp_gen_lp = updated_gen_lp
                    return [tmp_pool, tmp_trader, tmp_gen_lp]

        elif trade_decision['long']['direction'] == 'close':
            # Update the trader subtract the liquidity, add position with collateral, if position already exists subtract interest
            updated_trader = update_trader_close_long(tmp_trader, trade_decision, asset)
            updated_pool = update_pool_close_long(tmp_pool, updated_trader, asset, trade_decision, fees)
            if updated_pool != -1:
                #updated_pool, updated_gen_lp = update_gen_lp(updated_pool, tmp_gen_lp, fees[0], trade_decision['long']['interest_paid'], asset, asset_prices)
                updated_gen_lp = update_gen_lp(tmp_gen_lp, fees[0], trade_decision['long']['interest_paid'], asset)
                tmp_pool = updated_pool
                tmp_trader = updated_trader
                tmp_gen_lp = updated_gen_lp
                return [tmp_pool, tmp_trader, tmp_gen_lp]
            
    return None

def execute_short(pool, trader, gen_lp, trade_decision, fees, asset, timestep):
    tmp_pool = copy.deepcopy(pool)
    tmp_trader = copy.deepcopy(trader)
    tmp_gen_lp = copy.deepcopy(gen_lp)

    if trade_decision['short'] != None:
        if trade_decision['short']['direction'] == 'open':
            # Update the trader subtract the liquidity, add position with collateral, if position already exists subtract interest
            updated_trader = update_trader_open_short(tmp_trader, trade_decision, fees, asset, timestep)
            if updated_trader != -1:
                # Update the pool
                updated_pool = update_pool_open_short(tmp_pool, updated_trader, asset, trade_decision, fees)
                if updated_pool != -1:
                    # Update the genesis lp and pool
                    # updated_pool, updated_gen_lp = update_gen_lp(updated_pool, tmp_gen_lp, fees[1], trade_decision['short']['interest_paid'], trade_decision['short']['denomination'], asset_prices)
                    updated_gen_lp = update_gen_lp(tmp_gen_lp, fees[1], trade_decision['short']['interest_paid'], trade_decision['short']['denomination'])
                    tmp_pool = updated_pool
                    tmp_trader = updated_trader
                    tmp_gen_lp = updated_gen_lp
                    return [tmp_pool, tmp_trader, tmp_gen_lp]

        elif trade_decision['short']['direction'] == 'close':
            # Update the trader subtract the liquidity, add position with collateral, if position already exists subtract interest
            updated_trader = update_trader_close_short(tmp_trader, trade_decision, asset)
            updated_pool = update_pool_close_short(tmp_pool, updated_trader, asset, trade_decision, fees)
            if updated_pool != -1:
                # updated_pool, updated_gen_lp = update_gen_lp(updated_pool, tmp_gen_lp, fees[1], trade_decision['short']['interest_paid'], trade_decision['short']['denomination'], asset_prices)
                updated_gen_lp = update_gen_lp(tmp_gen_lp, fees[1], trade_decision['short']['interest_paid'], trade_decision['short']['denomination'])
                tmp_pool = updated_pool
                tmp_trader = updated_trader
                tmp_gen_lp = updated_gen_lp
                return [tmp_pool, tmp_trader, tmp_gen_lp]
            
    return None