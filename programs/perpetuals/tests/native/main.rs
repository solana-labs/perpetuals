pub mod adapters;
pub mod test_instructions;
pub mod tests_suite;
pub mod utils;

#[tokio::test]
pub async fn test_integration() {
    println!(">>>> lm_minting");
    tests_suite::lm_minting::lm_minting().await;

    println!(">>>> basic_interactions");
    tests_suite::basic_interactions().await;

    println!(">>>> fixed_fees");
    tests_suite::liquidity::fixed_fees().await;
    println!(">>>> insuffisient_fund");
    tests_suite::liquidity::insuffisient_fund().await;
    println!(">>>> min_max_ratio");
    tests_suite::liquidity::min_max_ratio().await;

    println!(">>>> min_max_leverage");
    tests_suite::position::min_max_leverage().await;
    println!(">>>> liquidate_position");
    tests_suite::position::liquidate_position().await;
    println!(">>>> max_user_profit");
    tests_suite::position::max_user_profit().await;

    println!(">>>> staking_rewards_generation");
    tests_suite::staking::staking_rewards_generation().await;
    println!(">>>> liquid_staking");
    tests_suite::staking::liquid_staking().await;
    println!(">>>> locked_staking_30d");
    tests_suite::staking::locked_staking_30d().await;
    tests_suite::staking::multiple_stakers_get_correct_rewards().await;
    tests_suite::staking::liquid_staking_overlap().await;
    tests_suite::staking::liquid_staking_overlap_remove_less_than_overlap().await;
    tests_suite::staking::liquid_staking_overlap_remove_same_as_overlap().await;
    tests_suite::staking::liquid_staking_overlap_remove_more_than_overlap().await;

    // Long tests
    tests_suite::staking::resolved_round_overflow().await;
    tests_suite::staking::auto_claim().await;

    tests_suite::lp_token::lp_token_price().await;
}
