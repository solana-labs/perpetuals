pub mod instructions;
pub mod tests_suite;
pub mod utils;

#[tokio::test]
pub async fn test_integration() {
    tests_suite::basic_interactions().await;
    
    tests_suite::swap::insuffisient_fund().await;

    tests_suite::add_remove_liquidity::fixed_fees().await;
    tests_suite::add_remove_liquidity::insuffisient_fund().await;
    tests_suite::add_remove_liquidity::min_max_ratio().await;
}