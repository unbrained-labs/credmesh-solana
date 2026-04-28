use anchor_lang::prelude::*;

#[error_code]
pub enum CredmeshError {
    #[msg("Insufficient idle liquidity in pool — deployed capital is locked")]
    InsufficientIdleLiquidity,
    #[msg("Receivable expired or not yet valid")]
    ReceivableExpired,
    #[msg("Receivable PDA stale — refresh required")]
    ReceivableStale,
    #[msg("Advance amount exceeds receivable cap (max_advance_pct_bps or max_advance_abs)")]
    AdvanceExceedsCap,
    #[msg("Advance amount exceeds reputation-derived credit limit")]
    AdvanceExceedsCredit,
    #[msg("Provided reputation PDA does not match expected derivation")]
    ReputationPdaMismatch,
    #[msg("Provided receivable PDA does not match expected derivation")]
    ReceivablePdaMismatch,
    #[msg("ed25519 verification missing or wrong format in tx")]
    Ed25519Missing,
    #[msg("ed25519 verified signer is not in CredMesh allowlist")]
    Ed25519SignerUnknown,
    #[msg("Memo nonce in payment tx does not match consumed PDA nonce")]
    MemoNonceMismatch,
    #[msg("Memo program instruction not found in tx")]
    MemoMissing,
    #[msg("Advance not yet within settlement window")]
    NotSettleable,
    #[msg("Advance not yet within liquidation grace period")]
    NotLiquidatable,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Governance signer required for this instruction")]
    GovernanceRequired,
    #[msg("Pending params not yet executable — timelock not satisfied")]
    PendingParamsNotReady,
    #[msg("No pending params to execute")]
    NoPendingParams,
    #[msg("Replay detected: ConsumedPayment PDA already exists for this receivable_id")]
    ReplayDetected,
    #[msg("Advance already settled or liquidated")]
    InvalidAdvanceState,
    #[msg("Pool is paused — only init/withdraw allowed; advance issuance is never paused")]
    PoolPaused,
    #[msg("Pause cannot be applied to advance issuance — design invariant")]
    PauseScopeViolation,
    #[msg("Source signer caps exceeded for ed25519 path")]
    SignerCapsExceeded,
    #[msg("Waterfall sum mismatch — rounding drift detected")]
    WaterfallSumMismatch,
}
