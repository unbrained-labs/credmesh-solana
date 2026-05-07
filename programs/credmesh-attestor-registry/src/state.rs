use anchor_lang::prelude::*;

pub use credmesh_shared::seeds::{ALLOWED_SIGNER_SEED, ATTESTOR_CONFIG_SEED};

/// Per-program config. Single PDA at `[ATTESTOR_CONFIG_SEED]`. Stores the
/// governance vault pubkey (a Squads vault) — every state-changing ix on
/// this program is gated on a Squads CPI introspection check verifying
/// the outer tx is co-signed against this vault.
#[account]
pub struct AttestorConfig {
    pub bump: u8,
    /// Squads vault PDA. Compared against the inner ix authority via
    /// `credmesh_shared::ix_introspection::require_squads_governance_cpi`.
    pub governance: Pubkey,
}

impl AttestorConfig {
    pub const SIZE: usize = 8 + 1 + 32;
}

/// One whitelisted bridge signer per AllowedSigner PDA. The signer's
/// ed25519 public key is the seed source: PDA at
/// `[ALLOWED_SIGNER_SEED, signer.as_ref()]`.
///
/// `kind` tags the attestation type the signer is authorized to produce.
/// v1 has a single kind: `AttestorKind::CreditBridge` (relays EVM
/// reputation snapshots into Solana request_advance via
/// `ed25519_credit_message`).
#[account]
pub struct AllowedSigner {
    pub bump: u8,
    pub signer: Pubkey,
    pub kind: u8,
    /// Wall-clock timestamp the signer was added. Audit trail only.
    pub added_at: i64,
}

impl AllowedSigner {
    pub const SIZE: usize = 8 + 1 + 32 + 1 + 8;
}
