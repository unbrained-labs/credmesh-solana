use anchor_lang::prelude::*;

#[error_code]
pub enum OracleError {
    #[msg("Caller is not the worker authority")]
    NotWorkerAuthority,
    #[msg("Caller is not the governance authority")]
    NotGovernance,
    #[msg("ed25519 verification missing or wrong format in tx")]
    Ed25519Missing,
    #[msg("ed25519 signer is not in allowed-signers registry")]
    SignerNotAllowed,
    #[msg("Per-receivable cap exceeded for this signer")]
    PerReceivableCapExceeded,
    #[msg("Per-period cap exceeded for this signer")]
    PerPeriodCapExceeded,
    #[msg("Receivable expired")]
    ReceivableExpired,
    #[msg("Math overflow")]
    MathOverflow,
}
