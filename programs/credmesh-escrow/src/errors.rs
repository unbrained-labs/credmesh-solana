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
    #[msg("Agent asset is not owned by the recognized agent registry program")]
    InvalidAgentAsset,
    #[msg("Signer is not the owner or delegate of agent_asset")]
    AgentBindingMismatch,
    #[msg("ed25519 verification missing or wrong format in tx")]
    Ed25519Missing,
    #[msg("ed25519 verified signer is not in CredMesh allowlist")]
    Ed25519SignerUnknown,
    #[msg("ed25519 offsets reference a different instruction than the verify ix")]
    Ed25519OffsetMismatch,
    #[msg("ed25519 message bytes do not match expected (receivable_id || agent || amount || expires_at || nonce)")]
    Ed25519MessageMismatch,
    #[msg("Memo nonce in payment tx does not match consumed PDA nonce")]
    MemoNonceMismatch,
    #[msg("Memo program instruction not found in tx")]
    MemoMissing,
    #[msg("Cranker is not authorized to call this instruction")]
    InvalidPayer,
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
    #[msg("Replay detected: ConsumedPayment PDA does not match advance.agent or already exists")]
    ReplayDetected,
    #[msg("Advance already settled or liquidated")]
    InvalidAdvanceState,
    #[msg("Source signer caps exceeded for ed25519 path")]
    SignerCapsExceeded,
    #[msg("Waterfall sum mismatch — rounding drift detected")]
    WaterfallSumMismatch,
    #[msg("Late days exceed maximum cap")]
    LateDaysExceeded,
    #[msg("Payment amount is less than total owed (principal + fee + late penalty)")]
    InsufficientPayment,
}
