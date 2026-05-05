use anchor_lang::prelude::*;

#[event]
pub struct AgentRegistered {
    pub agent: Pubkey,
    pub reputation_pda: Pubkey,
    pub credit_score: u32,
    pub credit_limit_atoms: u64,
    pub trust_score: u32,
    pub identity_registered: bool,
}

#[event]
pub struct CreditProfileUpdated {
    pub agent: Pubkey,
    pub credit_score: u32,
    pub credit_limit_atoms: u64,
    pub outstanding_balance_atoms: u64,
    pub repaid_advances: u32,
    pub defaulted_advances: u32,
}

#[event]
pub struct AdvanceRecorded {
    pub agent: Pubkey,
    pub principal_atoms: u64,
    pub outstanding_after_atoms: u64,
}

#[event]
pub struct SettlementRecorded {
    pub agent: Pubkey,
    pub principal_atoms: u64,
    pub outstanding_after_atoms: u64,
    pub credit_limit_after_atoms: u64,
    pub repaid_advances_after: u32,
}

#[event]
pub struct DefaultRecorded {
    pub agent: Pubkey,
    pub principal_atoms: u64,
    pub outstanding_after_atoms: u64,
    pub credit_limit_after_atoms: u64,
    pub defaulted_advances_after: u32,
}

#[event]
pub struct NewFeedback {
    pub agent: Pubkey,
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
    pub agent: Pubkey,
    pub feedback_index: u64,
    pub responder: Pubkey,
    pub response_uri: String,
    pub response_hash: [u8; 32],
}

#[event]
pub struct FeedbackRevoked {
    pub agent: Pubkey,
    pub feedback_index: u64,
    pub revoked_by: Pubkey,
}
