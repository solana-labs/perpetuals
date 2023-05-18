pub mod adapters;
pub mod instructions;
pub mod tests_suite;
pub mod utils;

#[tokio::test]
pub async fn test_integration() {
    tests_suite::basic_interactions().await;

    tests_suite::swap::insuffisient_fund().await;

    tests_suite::liquidity::fixed_fees().await;
    tests_suite::liquidity::insuffisient_fund().await;
    tests_suite::liquidity::min_max_ratio().await;

    tests_suite::position::min_max_leverage().await;
    tests_suite::position::liquidate_position().await;
    tests_suite::position::max_user_profit().await;

    tests_suite::staking::test_staking_rewards_from_swap().await;
    tests_suite::staking::test_staking_rewards_from_open_and_close_position().await;
    tests_suite::staking::test_staking_rewards_from_add_and_remove_liquidity().await;
    tests_suite::staking::test_bounty_no_rewards().await;
    tests_suite::staking::test_bounty_phase_one().await;
}
