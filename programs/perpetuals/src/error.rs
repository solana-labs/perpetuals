//! Error types

use anchor_lang::prelude::*;

#[error_code]
pub enum PerpetualsError {
    #[msg("Account is not authorized to sign this instruction")]
    MultisigAccountNotAuthorized,
    #[msg("Account has already signed this instruction")]
    MultisigAlreadySigned,
    #[msg("This instruction has already been executed")]
    MultisigAlreadyExecuted,
    #[msg("Overflow in arithmetic operation")]
    MathOverflow,
    #[msg("Unsupported price oracle")]
    UnsupportedOracle,
    #[msg("Invalid oracle account")]
    InvalidOracleAccount,
    #[msg("Invalid oracle state")]
    InvalidOracleState,
    #[msg("Stale oracle price")]
    StaleOraclePrice,
    #[msg("Invalid oracle price")]
    InvalidOraclePrice,
    #[msg("Instruction is not allowed in production")]
    InvalidEnvironment,
    #[msg("Invalid pool state")]
    InvalidPoolState,
    #[msg("Invalid vest state")]
    InvalidVestState,
    #[msg("Invalid stake state")]
    InvalidStakeState,
    #[msg("Invalid custody state")]
    InvalidCustodyState,
    #[msg("Invalid collateral custody")]
    InvalidCollateralCustody,
    #[msg("Invalid position state")]
    InvalidPositionState,
    #[msg("Invalid staking round state")]
    InvalidStakingRoundState,
    #[msg("Invalid perpetuals config")]
    InvalidPerpetualsConfig,
    #[msg("Invalid pool config")]
    InvalidPoolConfig,
    #[msg("Invalid custody config")]
    InvalidCustodyConfig,
    #[msg("Insufficient token amount returned")]
    InsufficientAmountReturned,
    #[msg("Price slippage limit exceeded")]
    MaxPriceSlippage,
    #[msg("Position leverage limit exceeded")]
    MaxLeverage,
    #[msg("Custody amount limit exceeded")]
    CustodyAmountLimit,
    #[msg("Position amount limit exceeded")]
    PositionAmountLimit,
    #[msg("Token ratio out of range")]
    TokenRatioOutOfRange,
    #[msg("Token is not supported")]
    UnsupportedToken,
    #[msg("Instruction is not allowed at this time")]
    InstructionNotAllowed,
    #[msg("Token utilization limit exceeded")]
    MaxUtilization,
    #[msg("Governance program do not match Cortex's")]
    InvalidGovernanceProgram,
    #[msg("Governance realm do not match Cortex's")]
    InvalidGovernanceRealm,
    #[msg("Vesting unlock time is too close or passed")]
    InvalidVestingUnlockTime,
    #[msg("Invalid staking locking time")]
    InvalidStakingLockingTime,
    #[msg("Cannot found stake")]
    CannotFoundStake,
    #[msg("Stake is not resolved")]
    UnresolvedStake,
    #[msg("Reached bucket mint limit")]
    BucketMintLimit,
    #[msg("Genesis ALP add liquidity limit reached")]
    GenesisAlpLimitReached,
}
