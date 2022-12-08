// admin instructions
pub mod add_pool;
pub mod add_token;
pub mod init;
pub mod remove_pool;
pub mod remove_token;
pub mod set_admin_signers;
pub mod set_borrow_rate;
pub mod set_permissions;
pub mod set_token_config;
pub mod withdraw_fees;

// test instructions
pub mod set_test_oracle_price;
pub mod set_test_time;
pub mod test_init;

// public instructions
pub mod add_collateral;
pub mod add_liquidity;
pub mod close_position;
pub mod get_entry_price_and_fee;
pub mod get_exit_price_and_fee;
pub mod get_liquidation_price;
pub mod get_pnl;
pub mod get_swap_amount_and_fees;
pub mod liquidate;
pub mod open_position;
pub mod remove_collateral;
pub mod remove_liquidity;
pub mod swap;

// bring everything in scope
pub use add_pool::*;
pub use add_token::*;
pub use init::*;
pub use remove_pool::*;
pub use remove_token::*;
pub use set_admin_signers::*;
pub use set_borrow_rate::*;
pub use set_permissions::*;
pub use set_token_config::*;
pub use withdraw_fees::*;

pub use set_test_oracle_price::*;
pub use set_test_time::*;
pub use test_init::*;

pub use add_collateral::*;
pub use add_liquidity::*;
pub use close_position::*;
pub use get_entry_price_and_fee::*;
pub use get_exit_price_and_fee::*;
pub use get_liquidation_price::*;
pub use get_pnl::*;
pub use get_swap_amount_and_fees::*;
pub use liquidate::*;
pub use open_position::*;
pub use remove_collateral::*;
pub use remove_liquidity::*;
pub use swap::*;
