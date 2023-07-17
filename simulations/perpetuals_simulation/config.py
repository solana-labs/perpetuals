from cadCAD.configuration import Experiment
from cadCAD.configuration.utils import config_sim
from state_variables import genesis_states
from psub import partial_state_update_block
from sys_params import sys_params, initial_conditions

sim_config = config_sim (
    {
        'N': 1, # number of monte carlo runs
        'T': range(initial_conditions['num_of_min']), # number of timesteps
        'M': sys_params, # simulation parameters
    }
)

exp = Experiment()

exp.append_configs(
    sim_configs=sim_config,
    initial_state=genesis_states,
    partial_state_update_blocks=partial_state_update_block
)
