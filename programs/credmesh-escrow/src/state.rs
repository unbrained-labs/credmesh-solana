use anchor_lang::prelude::*;

pub use credmesh_shared::seeds::{ADVANCE_SEED, CONSUMED_SEED, POOL_SEED, TREASURY_SEED};

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
    pub timelock_seconds: i64,
    pub pending_params: Option<PendingParams>,
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
    pub source_kind: u8,
    pub source_signer: Option<Pubkey>,
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
