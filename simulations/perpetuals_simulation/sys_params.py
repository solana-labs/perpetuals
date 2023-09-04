
ic = []

ic.append({
        'genesis_traders': 200, # genesis number of traders
        'genesis_providers': 70, # genesis number of providers
        'num_of_min': 1439, # number of minutes of the simulation
        # 'num_of_min': 1439, max for event 1
        'pool_fees': {'open': 0.01, 'close': 0.01} # pool fees
    })

ic.append({
        'genesis_traders': 200, # genesis number of traders
        'genesis_providers': 70, # genesis number of providers
        'num_of_min': 1439, # number of minutes of the simulation
        # 'num_of_min': 1439, max for event 2
        'pool_fees': {'open': 0.01, 'close': 0.01} # pool fees
    })

ic.append({
        'genesis_traders': 200, # genesis number of traders
        'genesis_providers': 70, # genesis number of providers
        'num_of_min': 659, # number of minutes of the simulation
        # 'num_of_min': 659, max for event 3
        'pool_fees': {'open': 0.01, 'close': 0.01} # pool fees
    })

ic.append({
        'genesis_traders': 200, # genesis number of traders
        'genesis_providers': 70, # genesis number of providers
        'num_of_min': 659, # number of minutes of the simulation
        # 'num_of_min': 659, max for event 4
        'pool_fees': {'open': 0.01, 'close': 0.01} # pool fees
    })

ic.append({
        'genesis_traders': 200, # genesis number of traders
        'genesis_providers': 70, # genesis number of providers
        'num_of_min': 719, # number of minutes of the simulation
        # 'num_of_min': 719, max for event 5
        'pool_fees': {'open': 0.01, 'close': 0.01} # pool fees
    })

ic.append({
        'genesis_traders': 200, # genesis number of traders
        'genesis_providers': 70, # genesis number of providers
        'num_of_min': 719, # number of minutes of the simulation
        # 'num_of_min': 719, max for event 6
        'pool_fees': {'open': 0.01, 'close': 0.01} # pool fees
    })

ic.append({
        'genesis_traders': 200, # genesis number of traders
        'genesis_providers': 70, # genesis number of providers
        'num_of_min': 719, # number of minutes of the simulation
        # 'num_of_min': 719, max for event 7
        'pool_fees': {'open': 0.01, 'close': 0.01} # pool fees
    })

ic.append({
        'genesis_traders': 200, # genesis number of traders
        'genesis_providers': 70, # genesis number of providers
        'num_of_min': 719, # number of minutes of the simulation
        # 'num_of_min': 719, max for event 8
        'pool_fees': {'open': 0.01, 'close': 0.01} # pool fees
    })

initial_conditions = ic

sp = []

for i in range(1, 9):
    # print(i)
    sp.append({
        # protocol params
        'base_fee': [0.05], # base fee
        'ratio_mult': [2], # ratio mult
        'max_margin': [{'BTC': 50, 'ETH': 50, 'SOL': 50, 'USDC': 50, 'USDT': 50}], # max margin
        'liquidation_threshold': [{'BTC': 0.01, 'ETH': 0.01, 'SOL': 0.01, 'USDC': 0.01, 'USDT': 0.01}], # liquidation thresholds
        'rate_params': [[0.8, 0.1, 0.1]], # rate parameters
        'base_fees_swap': [{'BTC': 0.0002, 'ETH': 0.0002, 'SOL': 0.0002, 'USDC': 0.0001, 'USDT': 0.0001}], # base fees
        'om_fees_swap': [{'coins': [0.0015, 0.00075], 'stables': [0.0005, 0.00025]}], # om fees for swaps
        'lp_fees': [{'add_base_fee': 0.005, 'optimal_fee': 0.001, 'max_fee': 0.025, 'rm_base_fee': 0.0005}],
        # simulation params
        'trader_traction': [0.01], # traction for traders (change in amount of traders)
        'lp_traction': [0.01], # traction for lps (change in amount of lps)
        'trade_chance': [[0.0003, 0.9996]], # 1st value is the barrier for longs, second is for shorts
        'swap_chance': [[0.0003, 0.9997]], # chance of swapping in and swapping out tokens 
        'event': [str(i)], # the event to be used
    })

sys_params = sp