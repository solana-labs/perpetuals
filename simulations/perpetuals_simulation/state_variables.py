from parts.utilities.utils import * 
from sys_params import initial_conditions, sys_params

def generate_providers(init_cond, event):
    conditions = copy.deepcopy(init_cond)
    liquidity_providers = {}
    assets = ['BTC', 'ETH', 'SOL', 'USDC', 'USDT']
    asset_prices = fetch_asset_prices(assets, 0, event)
    init_liq = initial_liquidity(asset_prices)

    # initialize protocol provider
    liquidity_providers['genesis'] = {
            'id': 'genesis',
            'funds': {'BTC': 0, 'ETH': 0, 'SOL': 0, 'USDC': 0, 'USDT': 0},
            'liquidity': init_liq,
            'add_threshold': {'BTC': 1000000, 'SOL': 1000000, 'ETH': 1000000, 'USDC': 1000000, 'USDT': 1000000},
            'remove_threshold': {'BTC': 1000000, 'SOL': 1000000, 'ETH': 1000000, 'USDC': 1000000, 'USDT': 1000000},
            'pool_share': 100
        }

    for i in range(conditions['genesis_providers']):
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
    return liquidity_providers

def generate_traders(init_cond, event):
    conditions = copy.deepcopy(init_cond)
    traders = {}
    assets = ['BTC', 'ETH', 'SOL', 'USDC', 'USDT']
    asset_prices = fetch_asset_prices(assets, 0, event)

    for i in range(conditions['genesis_traders']):
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

def generate_pools(init_cond, event):
    conditions = copy.deepcopy(init_cond)
    pools = {}
    assets = ['BTC', 'ETH', 'SOL', 'USDC', 'USDT']
    asset_prices = fetch_asset_prices(assets, 0, event)
    init_liq = initial_liquidity(asset_prices)
    init_tvl = pool_tvl(init_liq, asset_prices)

    for i in range(1):
        pool = {
            'id': i,
            'assets': ['BTC', 'ETH', 'SOL', 'USDC', 'USDT'],
            'holdings': copy.deepcopy(init_liq),
            'oi_long': {'BTC': 0, 'SOL': 0, 'ETH': 0, 'USDC': 0, 'USDT': 0},
            'oi_short': {'BTC': 0, 'SOL': 0, 'ETH': 0, 'USDC': 0, 'USDT': 0},
            'short_interest': {'USDC': 0, 'USDT': 0},
            'open_pnl_long': {'BTC': 0, 'SOL': 0, 'ETH': 0, 'USDC': 0, 'USDT': 0},
            'open_pnl_short': {'BTC': 0, 'SOL': 0, 'ETH': 0, 'USDC': 0, 'USDT': 0},
            'volume': {'BTC': 0, 'SOL': 0, 'ETH': 0, 'USDC': 0, 'USDT': 0},
            'total_fees_collected': {'BTC': 0, 'SOL': 0, 'ETH': 0, 'USDC': 0, 'USDT': 0},
            'yield': {'BTC': 0.01, 'SOL': 0.01, 'ETH': 0.01, 'USDC': 0.01, 'USDT': 0.01},
            'target_ratios': {'BTC': 0.23, 'SOL': 0.05, 'ETH': 0.24, 'USDC': 0.3, 'USDT': 0.18},
            'min_ratio': {'BTC': 0.1, 'SOL': 0.03, 'ETH': 0.1, 'USDC': 0.25, 'USDT': 0.015},
            'max_ratio': {'BTC': 0.5, 'SOL': 0.12, 'ETH': 0.5, 'USDC': 0.4, 'USDT': 0.21},
            'lps': {"genesis": copy.deepcopy(init_liq)},
            'loan_book_longs': {},  # {agent_id: {token: {amount, collateral}}}
            'loan_book_shorts': {},  # {agent_id: {token: {amount, collateral}}}
            'utilization_mult': {'BTC': 0.01, 'SOL': 0.01, 'ETH': 0.01, 'USDC': 0.01, 'USDT': 0.01},
            'fees': conditions['pool_fees'],
            'lp_shares': 100,
            'tvl': copy.deepcopy(init_tvl),
            'pool_ratios': {'BTC': copy.deepcopy(init_liq['BTC']) * asset_prices['BTC'][0] / init_tvl, 'SOL': copy.deepcopy(init_liq['SOL']) * asset_prices['SOL'][0] / init_tvl, 'ETH': copy.deepcopy(init_liq['ETH']) * asset_prices['ETH'][0] / init_tvl, 'USDC': copy.deepcopy(init_liq['USDC']) * asset_prices['USDC'][0] / init_tvl, 'USDT': copy.deepcopy(init_liq['USDT']) * asset_prices['USDT'][0] / init_tvl},
            'contract_oi': {'BTC': {'oi_long': 0, 'weighted_price_long': 0, 'tot_collateral': 0, 'weighted_collateral_price': 0 , 'oi_short': 0, 'weighted_price_short': 0}, 'SOL': {'oi_long': 0, 'weighted_price_long': 0, 'tot_collateral': 0, 'weighted_collateral_price': 0 , 'oi_short': 0, 'weighted_price_short': 0}, 'ETH': {'oi_long': 0, 'weighted_price_long': 0, 'tot_collateral': 0, 'weighted_collateral_price': 0 , 'oi_short': 0, 'weighted_price_short': 0}},
        }
    pools[i] = pool
    return pools

gs = []

for i in range(8):
    event = sys_params[i]['event'][0]
    gs.append({
        'traders': generate_traders(initial_conditions[i], event),
        'liquidity_providers': generate_providers(initial_conditions[i], event),    
        'pools': generate_pools(initial_conditions[i], event),
        'liquidations': 0,
        'num_of_longs': 0,
        'num_of_shorts': 0,
        'num_of_swaps': 0
    })

genesis_states = gs
