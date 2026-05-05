use anchor_lang::prelude::*;

pub use credmesh_shared::seeds::REPUTATION_SEED;

pub const SCORE_DECIMALS: u8 = 18;
pub const EMA_WINDOW: u64 = 50;

/// Maximum standing credit line in USDC atoms (= $1000 with 6 decimals).
/// Mirrors EVM `MAX_REPUTATION_CREDIT_CAP` in `credit-worker/src/credit.ts:12`.
pub const MAX_CREDIT_LIMIT_ATOMS: u64 = 1_000 * 1_000_000;

/// Cap on `outstanding_balance` factored into the score-penalty (EVM clamps
/// to $100 in `credit.ts:39`). Above this, the penalty saturates so an agent
/// can't get score=0 from a single huge active advance.
pub const SCORE_OUTSTANDING_CAP_ATOMS: u64 = 100 * 1_000_000;

/// Cap on `average_completed_payout` factored into the score (EVM clamps to
/// $200 in `credit.ts:35`).
pub const SCORE_AVG_PAYOUT_CAP_ATOMS: u64 = 200 * 1_000_000;

#[account]
#[derive(InitSpace)]
pub struct AgentReputation {
    pub bump: u8,

    /// The agent's primary signing key. PDA seed is
    /// `[REPUTATION_SEED, agent.as_ref()]`. Was previously seeded by an MPL
    /// Core asset pubkey; the EVM-parity port keys directly off the agent's
    /// pubkey so MPL Core is opt-in (an agent can be a raw keypair).
    pub agent: Pubkey,

    // ── EVM-port fields (port of credit-worker/src/credit.ts:24-56) ──
    /// Computed credit score 0..100. Refreshed by `register_agent` and by
    /// every `record_settlement_outcome`/`record_advance_issued` write.
    pub credit_score: u32,
    /// Standing credit line in USDC atoms. Capped at `MAX_CREDIT_LIMIT_ATOMS`.
    /// Recomputed on every state-changing reputation event.
    pub credit_limit_atoms: u64,
    /// Sum of currently-deployed advance principals for this agent across
    /// every advance. Decremented on settle/liquidate, incremented on
    /// request_advance.
    pub outstanding_balance_atoms: u64,
    /// External trust score 0..100 (e.g. ERC-8004 / SAS attestation
    /// aggregator). Set at register, updated by writer.
    pub trust_score: u32,
    pub attestation_count: u32,
    pub cooperation_success_count: u32,
    pub successful_jobs: u32,
    pub failed_jobs: u32,
    pub repaid_advances: u32,
    pub defaulted_advances: u32,
    /// Mean payout of completed jobs, in USDC atoms.
    pub average_completed_payout_atoms: u64,
    /// True iff the agent has registered an external identity attestation
    /// (ERC-8004 mirror SAS, etc.). Adds a +10 floor to `credit_score`.
    pub identity_registered: bool,

    // ── Permissionless feedback log (DECISIONS Q4) ──
    pub feedback_count: u64,
    pub feedback_digest: [u8; 32],
    /// Smoothed feedback-only score (18-decimal fixed-point). Separate from
    /// `credit_score`; not used for underwriting in v1 (kept for the SAS /
    /// 8004 indexer ecosystem).
    pub score_ema: u128,
    pub default_count: u32,
    pub last_event_slot: u64,
}

/// Update payload for `update_agent_attestations` (writer-gated). Each field
/// is `Option` so the writer can update only the values they have new
/// evidence for. Mirrors the EVM credit-worker's incremental updates to its
/// AgentRecord via the `/credit/profile` endpoint, but on-chain.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default)]
pub struct AgentAttestationUpdate {
    pub trust_score: Option<u32>,
    pub attestation_count: Option<u32>,
    pub cooperation_success_count: Option<u32>,
    pub average_completed_payout_atoms: Option<u64>,
    pub identity_registered: Option<bool>,
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
