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
    #[msg("Trust score must be 0-100")]
    TrustScoreOutOfRange,
    #[msg("Caller is not the configured reputation_writer_authority")]
    UnauthorizedWriter,
    #[msg("Outstanding balance underflow — settle/liquidate amount exceeds tracked outstanding")]
    OutstandingUnderflow,
    #[msg("Identity proof invalid — MPL Core asset owner mismatch or wrong account-owner program")]
    IdentityProofInvalid,
}
