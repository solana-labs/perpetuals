from .utils import *


def add_providers(liquidity_providers, traction, timestep, event, start_date):
    lps = copy.deepcopy(liquidity_providers)

    assets = ['BTC', 'ETH', 'SOL', 'USDC', 'USDT']
    price_dict = fetch_asset_prices(assets, timestep, event, start_date)
    asset_prices = get_asset_prices(price_dict)

    next_prov = len(lps) + 1
    add_prov_choice = random.random()

    if add_prov_choice < traction:
        thresholds = {asset: np.random.uniform(low=0, high=0.1) for asset in assets}
        liquidity_provider = {
            'id': next_prov,
            'funds': {asset: np.random.uniform(low=100, high=5000)/asset_prices[f'{asset}'][0] for asset in assets},
            'liquidity': {asset: 0 for asset in assets},
            'add_threshold': thresholds,
            'remove_threshold': {asset: (thresholds[f'{asset}'] * 0.7) for asset in assets},
            'pool_share': 0
        }
        lps[next_prov] = liquidity_provider

    return lps

def add_traders(traders, traction, timestep, event, start_date):
    trs = copy.deepcopy(traders)
    assets = ['BTC', 'ETH', 'SOL', 'USDC', 'USDT']
    price_dict = fetch_asset_prices(assets, timestep, event, start_date)
    asset_prices = get_asset_prices(price_dict)

    next_tr = len(trs) + 1
    add_trad_choice = random.random()

    if add_trad_choice < traction:
        trader = {
            'id': next_tr,
            'liquidity': {asset: np.random.uniform(low=100, high=5000)/asset_prices[f'{asset}'][0] for asset in assets},  # Sample initial liquidity from some distribution
            'positions_long': {},  # {token: {quantity: 0, entry_price: 0, collateral: 0, timestep: 0}}
            'positions_short': {},  # {token: {quantity: 0, entry_price: 0, collateral: {amount: 0, denomination: "USDC"}, timestep: 0}}
            'PnL': 0,
            'avg_position_hold': np.random.uniform(low=1, high=10),
            'risk_factor': np.random.uniform(low=1, high=10)
        }
        trs[next_tr] = trader

    return trs