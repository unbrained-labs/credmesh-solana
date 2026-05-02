use anchor_lang::prelude::*;

pub use credmesh_shared::seeds::REPUTATION_SEED;

pub const SCORE_DECIMALS: u8 = 18;
pub const EMA_WINDOW: u64 = 50;

#[account]
#[derive(InitSpace)]
pub struct AgentReputation {
    pub bump: u8,
    pub agent_asset: Pubkey,
    pub feedback_count: u64,
    pub feedback_digest: [u8; 32],
    pub score_ema: u128,
    pub default_count: u32,
    pub last_event_slot: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct FeedbackInput {
    pub score: u8,
    pub value: u64,
    pub value_decimals: u8,
    pub reason_code: u16,
    pub feedback_uri: String,
    pub feedback_hash: [u8; 32],
    pub job_id: [u8; 32],
}
