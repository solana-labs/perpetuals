from parts.utilities.utils import * 
from sys_params import initial_conditions, sys_params

def generate_providers(n_providers):
    liquidity_providers = {}
    assets = ['BTC', 'ETH', 'SOL', 'USDC', 'USDT']
    asset_prices = fetch_asset_prices(assets, 0, sys_params['event'][0])

    # initialize protocol provider
    liquidity_providers['genesis'] = {
            'id': 'genesis',
            'funds': {'BTC': 0, 'ETH': 0, 'SOL': 0, 'USDC': 0, 'USDT': 0},
            'liquidity': copy.deepcopy(initial_conditions['initial_liquidity']),
            'add_threshold': {'BTC': 1000000, 'SOL': 1000000, 'ETH': 1000000, 'USDC': 1000000, 'USDT': 1000000},
            'remove_threshold': {'BTC': 1000000, 'SOL': 1000000, 'ETH': 1000000, 'USDC': 1000000, 'USDT': 1000000},
            'pool_share': 100
        }

    for i in range(n_providers):
        thresholds = {asset: np.random.uniform(low=0, high=0.1) for asset in assets}
        liquidity_provider = {
            'id': i,
            'funds': {asset: np.random.uniform(low=100, high=5000)/asset_prices[f'{asset}'][0] for asset in assets},
            'liquidity': {asset: 0 for asset in assets},
            'add_threshold': thresholds,
            'remove_threshold': {asset: (thresholds[f'{asset}'] * 0.7) for asset in assets},
            'pool_share': 0
        }
        liquidity_providers[i] = liquidity_provider
        #print("theresholds", thresholds)
    return liquidity_providers

def generate_traders(n_traders):
    traders = {}
    assets = ['BTC', 'ETH', 'SOL', 'USDC', 'USDT']
    asset_prices = fetch_asset_prices(assets, 0, sys_params['event'][0])

    for i in range(n_traders):
        trader = {
            'id': i,
            'liquidity': {asset: np.random.uniform(low=100, high=5000)/asset_prices[f'{asset}'][0] for asset in assets},  # Sample initial liquidity from some distribution
            'positions_long': {},  # {token: {quantity: 0, entry_price: 0, collateral: 0, nominal_collaterall: 0, timestep: 0}}
            'positions_short': {},  # {token: {quantity: 0, entry_price: 0, collateral: {amount: 0, denomination: "USDC"}, timestep: 0}}
            'PnL': 0,
            'avg_position_hold': np.random.uniform(low=1, high=100),
            'risk_factor': np.random.uniform(low=1, high=10)
        }
        traders[i] = trader
    return traders

def generate_pools(n_pools):
    pools = {}
    assets = ['BTC', 'ETH', 'SOL', 'USDC', 'USDT']
    asset_prices = fetch_asset_prices(assets, 0, sys_params['event'][0])
    init_liq = copy.deepcopy(initial_conditions['initial_liquidity'])
    init_tvl = pool_tvl(init_liq, asset_prices)

    for i in range(n_pools):
        #token_a, token_b = np.random.choice(tokens, size=2, replace=False)  # Choose two different tokens for the pool
        pool = {
            'id': i,
            'assets': ['BTC', 'ETH', 'SOL', 'USDC', 'USDT'],
            'holdings': copy.deepcopy(initial_conditions['initial_liquidity']),
            'oi_long': {'BTC': 0, 'SOL': 0, 'ETH': 0, 'USDC': 0, 'USDT': 0},
            'oi_short': {'BTC': 0, 'SOL': 0, 'ETH': 0, 'USDC': 0, 'USDT': 0},
            'short_interest': {'USDC': 0, 'USDT': 0},
            'open_pnl_long': {'BTC': 0, 'SOL': 0, 'ETH': 0, 'USDC': 0, 'USDT': 0},
            'open_pnl_short': {'BTC': 0, 'SOL': 0, 'ETH': 0, 'USDC': 0, 'USDT': 0},
            'volume': {'BTC': 0, 'SOL': 0, 'ETH': 0, 'USDC': 0, 'USDT': 0},
            'total_fees_collected': {'BTC': 0, 'SOL': 0, 'ETH': 0, 'USDC': 0, 'USDT': 0},
            'yield': {'BTC': 0.01, 'SOL': 0.01, 'ETH': 0.01, 'USDC': 0.01, 'USDT': 0.01},
            'target_ratios': {'BTC': 0.2, 'SOL': 0.2, 'ETH': 0.2, 'USDC': 0.2, 'USDT': 0.2},
            'deviation': 0.05,
            'lps': {"genesis": copy.deepcopy(initial_conditions['initial_liquidity'])},
            'loan_book_longs': {},  # {agent_id: {token: {amount, collateral}}}
            'loan_book_shorts': {},  # {agent_id: {token: {amount, collateral}}}
            'utilization_mult': {'BTC': 0.01, 'SOL': 0.01, 'ETH': 0.01, 'USDC': 0.01, 'USDT': 0.01},
            'fees': initial_conditions['pool_fees'],
            'lp_shares': 100,
            'tvl': init_tvl,
            'pool_ratios': {'BTC': init_liq['BTC'] * asset_prices['BTC'][0] / init_tvl, 'SOL': init_liq['SOL'] * asset_prices['SOL'][0] / init_tvl, 'ETH': init_liq['ETH'] * asset_prices['ETH'][0] / init_tvl, 'USDC': init_liq['USDC'] * asset_prices['USDC'][0] / init_tvl, 'USDT': init_liq['USDT'] * asset_prices['USDT'][0] / init_tvl},
            'contract_oi': {'BTC': {'oi_long': 0, 'avg_price_long': 0, 'tot_collateral': 0, 'avg_collateral_price': 0 , 'oi_short': 0, 'avg_price_short': 0}, 'SOL': {'oi_long': 0, 'avg_price_long': 0, 'tot_collateral': 0, 'avg_collateral_price': 0 , 'oi_short': 0, 'avg_price_short': 0}, 'ETH': {'oi_long': 0, 'avg_price_long': 0, 'tot_collateral': 0, 'avg_collateral_price': 0 , 'oi_short': 0, 'avg_price_short': 0}},
        }
    pools[i] = pool
    return pools

genesis_states = {
    'traders': generate_traders(copy.deepcopy(initial_conditions['genesis_traders'])),
    'liquidity_providers': generate_providers(copy.deepcopy(initial_conditions['genesis_providers'])),    
    'pools': generate_pools(1),
    'liquidations': 0,
    'num_of_longs': 0,
    'num_of_shorts': 0,
    'num_of_swaps': 0
    # 'oracle_attack': False
}
