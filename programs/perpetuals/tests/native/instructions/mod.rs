pub mod test_add_custody;
pub mod test_add_liquidity;
pub mod test_add_pool;
pub mod test_add_stake;
pub mod test_add_vest;
pub mod test_claim_stakes;
pub mod test_claim_vest;
pub mod test_close_position;
pub mod test_get_lp_token_price;
pub mod test_init;
pub mod test_init_staking;
pub mod test_liquidate;
pub mod test_open_position;
pub mod test_remove_liquidity;
pub mod test_remove_stake;
pub mod test_resolve_locked_stakes;
pub mod test_resolve_staking_round;
pub mod test_set_custody_config;
pub mod test_set_custom_oracle_price;
pub mod test_swap;

pub use {
    test_add_custody::*, test_add_liquidity::*, test_add_pool::*, test_add_stake::*,
    test_add_vest::*, test_claim_stakes::*, test_claim_vest::*, test_close_position::*,
    test_get_lp_token_price::*, test_init::*, test_init_staking::*, test_liquidate::*,
    test_open_position::*, test_remove_liquidity::*, test_remove_stake::*,
    test_resolve_locked_stakes::*, test_resolve_staking_round::*, test_set_custody_config::*,
    test_set_custom_oracle_price::*, test_swap::*,
};
