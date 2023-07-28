
from .utilities.trac_mech import *

def more_agents_policy(params, substep, state_history, previous_state):

    traders = copy.deepcopy(previous_state['traders'])
    liquidity_providers = copy.deepcopy(previous_state['liquidity_providers'])
    timestep = previous_state['timestep']
    print(timestep, 'traction')

    # generate new liquidity providers
    liquidity_providers = add_providers(liquidity_providers, params['lp_traction'], timestep, params['event'])
    # generate new traders
    traders = add_traders(traders, params['trader_traction'], timestep, params['event'])

    action = {
        'traders': traders,
        'liquidity_providers': liquidity_providers
    }

    return action

def more_providers_update(params, substep, state_history, previous_state, policy):
    key = 'liquidity_providers'
    value = policy['liquidity_providers']
    return (key, value)

def more_traders_update(params, substep, state_history, previous_state, policy):
    key = 'traders'
    value = policy['traders']
    return (key, value)