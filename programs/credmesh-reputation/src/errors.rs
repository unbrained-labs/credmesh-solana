use anchor_lang::prelude::*;

#[error_code]
pub enum ReputationError {
    #[msg("Score must be 0-100")]
    InvalidScore,
    #[msg("Feedback URI exceeds maximum length")]
    UriTooLong,
    #[msg("Feedback already exists at this index")]
    FeedbackExists,
    #[msg("Caller is not the original feedback signer; cannot revoke")]
    NotOriginalSigner,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("OracleConfig cross-program account did not match expected derivation")]
    OracleConfigMismatch,
    #[msg("Instruction not implemented in v1")]
    NotImplemented,
}
