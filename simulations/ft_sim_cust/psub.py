from parts.liquidity import *
from parts.trading import *
from parts.traction import *

partial_state_update_block = [
    {
        # liquidity.py
        'policies': {
            'liquidity': liquidity_policy,
        },
        'variables': {
            'liquidity_providers': liquidity_providers_update,
            'pools': pool_liquidity_update,
        }
    },
    {
        # trading.py
        'policies': {
            'trading': trading_policy,
        },
        'variables': {
            'traders': traders_update,
            'pools': pool_trading_update,
            'liquidations': liquidations_uodate,
            'liquidity_providers': distribution_providers_update,
            'num_of_longs': num_of_longs_update,
            'num_of_shorts': num_of_shorts_update,
            'num_of_swaps': num_of_swaps_update,
            # 'oracle_attack': oracle_attack_update
        }
    },
    {
        # traction.py
        'policies': {
            'generate_more_agents': more_agents_policy,
        },
        'variables': {
            'liquidity_providers': more_providers_update,
            'traders': more_traders_update,
        }
    },
]