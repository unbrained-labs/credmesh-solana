use anchor_lang::prelude::*;

pub const RECEIVABLE_SEED: &[u8] = b"receivable";
pub const ALLOWED_SIGNER_SEED: &[u8] = b"allowed_signer";
pub const ORACLE_CONFIG_SEED: &[u8] = b"oracle_config";

pub const MAX_STALENESS_SLOTS: u64 = 5_400;

#[account]
pub struct OracleConfig {
    pub bump: u8,
    pub governance: Pubkey,
    pub worker_authority: Pubkey,
    pub worker_max_per_tx: u64,
    pub worker_max_per_period: u64,
    pub worker_period_seconds: i64,
    pub worker_period_start: i64,
    pub worker_period_used: u64,
}

impl OracleConfig {
    pub const SIZE: usize = 8 + 1 + 32 * 2 + 8 * 5 + 32;
}

#[account]
pub struct Receivable {
    pub bump: u8,
    pub agent: Pubkey,
    pub source_id: [u8; 32],
    pub source_kind: u8,
    pub source_signer: Option<Pubkey>,
    pub amount: u64,
    pub expires_at: i64,
    pub last_updated_slot: u64,
    pub authority: Pubkey,
}

impl Receivable {
    pub const SIZE: usize = 8 + 1 + 32 + 32 + 1 + (1 + 32) + 8 * 3 + 32 + 32;
}

#[account]
pub struct AllowedSigner {
    pub bump: u8,
    pub signer: Pubkey,
    pub kind: u8,
    pub max_per_receivable: u64,
    pub max_per_period: u64,
    pub period_seconds: i64,
    pub period_start: i64,
    pub period_used: u64,
}

impl AllowedSigner {
    pub const SIZE: usize = 8 + 1 + 32 + 1 + 8 * 5 + 32;
}
