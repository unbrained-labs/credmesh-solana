use anchor_lang::prelude::*;

#[event]
pub struct OracleInitialized {
    pub governance: Pubkey,
    pub worker_authority: Pubkey,
}

#[event]
pub struct ReceivableUpdated {
    pub agent: Pubkey,
    pub source_id: [u8; 32],
    pub source_kind: u8,
    pub source_signer: Option<Pubkey>,
    pub amount: u64,
    pub expires_at: i64,
    pub authority: Pubkey,
}

#[event]
pub struct AllowedSignerAdded {
    pub signer: Pubkey,
    pub kind: u8,
    pub max_per_receivable: u64,
    pub max_per_period: u64,
}

#[event]
pub struct AllowedSignerRemoved {
    pub signer: Pubkey,
}
