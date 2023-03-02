// admin instructions
pub mod add_custody;
pub mod add_pool;
pub mod init;
pub mod remove_custody;
pub mod remove_pool;
pub mod set_admin_signers;
pub mod set_custody_config;
pub mod set_permissions;
pub mod upgrade_custody;
pub mod withdraw_fees;

// test instructions
pub mod set_test_oracle_price;
pub mod set_test_time;
pub mod test_init;

// public instructions
pub mod add_collateral;
pub mod add_liquidity;
pub mod close_position;
pub mod get_assets_under_management;
pub mod get_entry_price_and_fee;
pub mod get_exit_price_and_fee;
pub mod get_liquidation_price;
pub mod get_liquidation_state;
pub mod get_oracle_price;
pub mod get_pnl;
pub mod get_swap_amount_and_fees;
pub mod liquidate;
pub mod open_position;
pub mod remove_collateral;
pub mod remove_liquidity;
pub mod swap;

// bring everything in scope
pub use add_custody::*;
pub use add_pool::*;
pub use init::*;
pub use remove_custody::*;
pub use remove_pool::*;
pub use set_admin_signers::*;
pub use set_custody_config::*;
pub use set_permissions::*;
pub use upgrade_custody::*;
pub use withdraw_fees::*;

pub use set_test_oracle_price::*;
pub use set_test_time::*;
pub use test_init::*;

pub use add_collateral::*;
pub use add_liquidity::*;
pub use close_position::*;
pub use get_assets_under_management::*;
pub use get_entry_price_and_fee::*;
pub use get_exit_price_and_fee::*;
pub use get_liquidation_price::*;
pub use get_liquidation_state::*;
pub use get_oracle_price::*;
pub use get_pnl::*;
pub use get_swap_amount_and_fees::*;
pub use liquidate::*;
pub use open_position::*;
pub use remove_collateral::*;
pub use remove_liquidity::*;
pub use swap::*;
