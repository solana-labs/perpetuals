

initial_conditions = {
    'genesis_traders': 20,
    'genesis_providers': 10,
    'num_of_hrs': 10,
    'initial_liquidity': {'BTC': 1, 'ETH': 15, 'SOL': 1500, 'USDC': 30000, 'USDT': 30000},
}

sys_params = {
    # protocol params
    'base_fee': [0.05],
    'ratio_mult': [2],
    'max_margin': [50],
    'liquidation_threshold': [0.02],
    'rate_params': [[0.8, 0.1, 0.1]], # we need to figure this part out and optimize it
    'base_fees_swap': [{'BTC': 0.00025, 'ETH': 0.00025, 'SOL': 0.00015, 'USDC': 0.0001, 'USDT': 0.0001}],
    'om_fees_swap': [[0.01, 0.005]],
    # simulation params
    'trader_traction': [0.05],
    'lp_traction': [0.03],
    'trade_chance': [[0.1, 0.9]], # 1st value is the barrier for longs, second is for shorts
    'swap_chance': [[0.1, 0.9]], # chance of swapping in and swapping out tokens 
}