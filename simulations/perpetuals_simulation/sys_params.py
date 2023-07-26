

initial_conditions = {
    'genesis_traders': 30, # genesis number of traders
    'genesis_providers': 10, # genesis number of providers
    'num_of_min': 719, # number of minutes of the simulation
    # 'num_of_min': 1439, max for event 1
    # 'num_of_min': 1439, max for event 2
    # 'num_of_min': 659, max for event 3
    # 'num_of_min': 659, max for event 4
    # 'num_of_min': 719, max for event 5
    # 'num_of_min': 719, max for event 6
    # 'num_of_min': 719, max for event 7
    # 'num_of_min': 719, max for event 8
    'initial_liquidity': {'BTC': 1, 'ETH': 13, 'SOL': 625, 'USDC': 20000, 'USDT': 20000}, # initial pool liquidity
    'pool_fees': {'open': 0.01, 'close': 0.01} # pool fees
}

sys_params = {
    # protocol params
    'base_fee': [0.05], # base fee
    'ratio_mult': [2], # ratio mult
    'max_margin': [{'BTC': 50, 'ETH': 50, 'SOL': 50, 'USDC': 50, 'USDT': 50}], # max margin
    'liquidation_threshold': [{'BTC': 0.02, 'ETH': 0.02, 'SOL': 0.02, 'USDC': 0.02, 'USDT': 0.02}], # liquidation thresholds
    'rate_params': [[0.8, 0.1, 0.1]], # rate parameters
    'base_fees_swap': [{'BTC': 0.00025, 'ETH': 0.00025, 'SOL': 0.00015, 'USDC': 0.0001, 'USDT': 0.0001}], # base fees
    'om_fees_swap': [[0.01, 0.005]], # om fees for swaps
    # simulation params
    'trader_traction': [0.0], # traction for traders (change in amount of traders)
    'lp_traction': [0.0], # traction for lps (change in amount of lps)
    'trade_chance': [[0.01, 0.99]], # 1st value is the barrier for longs, second is for shorts
    'swap_chance': [[0.01, 0.99]], # chance of swapping in and swapping out tokens 
    'event': ['8'], # the event to be used
}