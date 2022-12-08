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
    #[msg("Invalid custody state")]
    InvalidCustodyState,
    #[msg("Invalid position state")]
    InvalidPositionState,
    #[msg("Invalid perpetuals config")]
    InvalidPerpetualsConfig,
    #[msg("Invalid token config")]
    InvalidTokenConfig,
    #[msg("Not enough tokens returned")]
    InsufficientAmountReturned,
    #[msg("Price slippage limit exceeded")]
    MaxPriceSlippage,
    #[msg("Position leverage limit exceeded")]
    MaxLeverage,
    #[msg("Pool amount limit exceeded")]
    PoolAmountLimit,
    #[msg("Token is not supported")]
    UnsupportedToken,
    #[msg("Instruction is not allowed at this time")]
    InstructionNotAllowed,
}
