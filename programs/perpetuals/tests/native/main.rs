pub mod adapters;
pub mod test_instructions;
pub mod tests_suite;
pub mod utils;

#[tokio::test]
pub async fn test_integration() {
    tests_suite::lm_minting::mint_lm_tokens_from_bucket().await;

    tests_suite::basic_interactions().await;

    tests_suite::liquidity::fixed_fees().await;
    tests_suite::liquidity::insuffisient_fund().await;
    tests_suite::liquidity::min_max_ratio().await;
    tests_suite::liquidity::genesis().await;

    tests_suite::position::min_max_leverage().await;
    tests_suite::position::liquidate_position().await;
    tests_suite::position::max_user_profit().await;

    tests_suite::staking::staking_rewards_generation().await;
    tests_suite::staking::liquid_staking().await;
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
