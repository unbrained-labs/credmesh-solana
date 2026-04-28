use anchor_lang::prelude::*;

#[event]
pub struct ReputationInitialized {
    pub agent_asset: Pubkey,
    pub reputation_pda: Pubkey,
}

#[event]
pub struct NewFeedback {
    pub agent_asset: Pubkey,
    pub feedback_index: u64,
    pub attestor: Pubkey,
    pub score: u8,
    pub value: u64,
    pub value_decimals: u8,
    pub reason_code: u16,
    pub feedback_uri: String,
    pub feedback_hash: [u8; 32],
    pub job_id: [u8; 32],
    pub digest_after: [u8; 32],
    pub score_ema_after: u128,
}

#[event]
pub struct FeedbackResponse {
    pub agent_asset: Pubkey,
    pub feedback_index: u64,
    pub responder: Pubkey,
    pub response_uri: String,
    pub response_hash: [u8; 32],
}

#[event]
pub struct FeedbackRevoked {
    pub agent_asset: Pubkey,
    pub feedback_index: u64,
    pub revoked_by: Pubkey,
}
