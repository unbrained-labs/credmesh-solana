use anchor_lang::prelude::*;

pub use credmesh_shared::seeds::{
    ADVANCE_SEED, CONSUMED_SEED, ISSUANCE_LEDGER_SEED, POOL_SEED,
};

pub const PROTOCOL_FEE_BPS: u16 = 1500;
pub const BPS_DENOMINATOR: u64 = 10_000;

pub const CLAIM_WINDOW_SECONDS: i64 = 7 * 24 * 60 * 60;
pub const LIQUIDATION_GRACE_SECONDS: i64 = 14 * 24 * 60 * 60;

pub const VIRTUAL_ASSETS_OFFSET: u64 = 1_000_000;
pub const VIRTUAL_SHARES_OFFSET: u64 = 1_000_000_000;

pub const MAX_LATE_DAYS: u32 = 365;

/// Floor on advance amounts. 1 USDC = 1_000_000 atoms. Below this, the late
/// penalty math (0.1%/day) truncates to zero and tx fees dominate.
pub const MIN_ADVANCE_ATOMS: u64 = 1_000_000;

/// Rolling-window length for the per-agent issuance ledger. Caps how much
/// principal a single agent can pull in any 24-hour window — the on-chain
/// blast-radius bound when a bridge ed25519 signer key is compromised
/// (which is *additionally* bounded by the 15-min attestation TTL plus
/// governance-revocable AllowedSigner whitelist).
pub const AGENT_WINDOW_SECONDS: i64 = 24 * 60 * 60;

#[account]
#[derive(InitSpace)]
pub struct Pool {
    pub bump: u8,
    pub asset_mint: Pubkey,
    pub usdc_vault: Pubkey,
    pub share_mint: Pubkey,
    pub treasury_ata: Pubkey,
    pub governance: Pubkey,
    pub total_assets: u64,
    pub total_shares: u64,
    pub deployed_amount: u64,
    pub accrued_protocol_fees: u64,
    pub fee_curve: FeeCurve,
    pub max_advance_pct_bps: u16,
    pub max_advance_abs: u64,
    /// Per-agent rolling-window issuance cap (USDC atoms) over the trailing
    /// `AGENT_WINDOW_SECONDS`. `0` means feature-disabled (the on-chain cap
    /// is opt-in by governance — backwards-compat for fresh devnet pools
    /// initialized before the operator decides on a value). Mainnet
    /// configuration MUST set this > 0 — a typical bound is 5-10× the
    /// expected per-agent daily borrow, low enough to bound bridge-key-
    /// compromise damage but high enough not to throttle legitimate use.
    pub agent_window_cap: u64,
    pub timelock_seconds: i64,
    /// Cluster identifier for cross-chain replay defense on ed25519 credit
    /// attestations. Matches `credmesh_shared::ed25519_credit_message::
    /// CHAIN_ID_*`. Set at `init_pool` and never mutated. Devnet attestations
    /// MUST NOT verify against a mainnet pool and vice versa, even when the
    /// same bridge signer is whitelisted on both clusters.
    pub chain_id: u64,
    pub pending_params: Option<PendingParams>,
}

impl Pool {
    /// PDA seeds for signing as the pool authority. Caller stack-allocates
    /// the bump array (Solana's `&[u8]` slot needs storage that outlives
    /// the CPI call). Pattern: `let bump = [pool.bump]; let seeds =
    /// pool.signer_seeds(&bump); let signer: &[&[&[u8]]] = &[&seeds];`.
    /// Centralizes the seed shape so a future seed change updates one site.
    #[inline]
    pub fn signer_seeds<'a>(&'a self, bump: &'a [u8; 1]) -> [&'a [u8]; 3] {
        [POOL_SEED, self.asset_mint.as_ref(), bump]
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default, InitSpace)]
pub struct FeeCurve {
    pub utilization_kink_bps: u16,
    pub base_rate_bps: u16,
    pub kink_rate_bps: u16,
    pub max_rate_bps: u16,
    pub duration_per_day_bps: u16,
    pub risk_premium_bps: u16,
    pub pool_loss_surcharge_bps: u16,
}

impl FeeCurve {
    /// Audit-MED #5: enforce internal ordering and BPS bounds before any
    /// curve is stored on a `Pool` (init_pool / propose_params). Without
    /// this, governance could pre-stage a curve where `kink_rate > max_rate`
    /// or `kink_bps > BPS_DENOMINATOR`. The fee math doesn't panic on
    /// such inputs but produces nonsense rates (saturating intermediaries
    /// hide the bug). Cheap to enforce up-front; costs nothing at runtime.
    ///
    /// Required invariants:
    ///   - `utilization_kink_bps <= BPS_DENOMINATOR`
    ///   - `base_rate_bps <= kink_rate_bps <= max_rate_bps`
    ///   - `max_rate_bps <= BPS_DENOMINATOR`
    pub fn validate(&self) -> Result<()> {
        require!(
            (self.utilization_kink_bps as u64) <= BPS_DENOMINATOR,
            crate::errors::CredmeshError::InvalidFeeCurve
        );
        require!(
            self.base_rate_bps <= self.kink_rate_bps,
            crate::errors::CredmeshError::InvalidFeeCurve
        );
        require!(
            self.kink_rate_bps <= self.max_rate_bps,
            crate::errors::CredmeshError::InvalidFeeCurve
        );
        require!(
            (self.max_rate_bps as u64) <= BPS_DENOMINATOR,
            crate::errors::CredmeshError::InvalidFeeCurve
        );
        Ok(())
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, InitSpace)]
pub struct PendingParams {
    pub fee_curve: FeeCurve,
    pub max_advance_pct_bps: u16,
    pub max_advance_abs: u64,
    pub agent_window_cap: u64,
    pub execute_after: i64,
}

#[account]
#[derive(InitSpace)]
pub struct Advance {
    pub bump: u8,
    pub agent: Pubkey,
    pub receivable_id: [u8; 32],
    pub principal: u64,
    pub fee_owed: u64,
    pub late_penalty_per_day: u64,
    pub issued_at: i64,
    pub expires_at: i64,
    /// The bridge signer whose ed25519 credit attestation underwrote
    /// this advance. Stored for audit trail and for the off-chain bridge
    /// service to correlate Solana settle/liquidate events back to EVM.
    pub attestor: Pubkey,
    pub state: AdvanceState,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug, InitSpace)]
pub enum AdvanceState {
    Issued,
    Settled,
    Liquidated,
}

impl Default for AdvanceState {
    fn default() -> Self {
        Self::Issued
    }
}

/// **Permanent** PDA per receivable_id. Never closed — closing it would re-open
/// a replay channel via close-then-reinit in the same tx (audit P0-5).
#[account]
#[derive(InitSpace)]
pub struct ConsumedPayment {
    pub bump: u8,
    pub nonce: [u8; 16],
    pub agent: Pubkey,
    pub created_at: i64,
}

// ProtocolTreasury is not a separate PDA in v1 — `Pool.treasury_ata` stores the
// destination ATA directly. The TREASURY_SEED constant is reserved for v2 if a
// dedicated PDA is ever needed (e.g., to track per-pool fee accrual on-chain
// independently of the ATA balance).

/// Per-agent rolling-window issuance ledger. Bounds the principal that any
/// single agent can pull in `AGENT_WINDOW_SECONDS`, defending against
/// bridge-key-compromise scenarios where an attacker could otherwise
/// saturate the pool against a small set of agents within the 15-min
/// attestation TTL window.
///
/// Update rule (in `request_advance`):
///   - if `now - window_start >= AGENT_WINDOW_SECONDS`, reset window:
///     `window_start = now`, `issued_in_window = 0`
///   - then `issued_in_window += amount`
///   - require `issued_in_window <= pool.agent_window_cap`
///
/// Init via `init_if_needed` (one-time per agent per pool, not a replay-
/// protection PDA — replay protection is `ConsumedPayment`, AUDIT P0-5).
#[account]
#[derive(InitSpace)]
pub struct AgentIssuanceLedger {
    pub bump: u8,
    pub agent: Pubkey,
    pub pool: Pubkey,
    pub window_start: i64,
    pub issued_in_window: u64,
}
