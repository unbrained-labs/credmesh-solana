use anchor_lang::prelude::*;

pub const POOL_SEED: &[u8] = b"pool";
pub const ADVANCE_SEED: &[u8] = b"advance";
pub const CONSUMED_SEED: &[u8] = b"consumed";
pub const TREASURY_SEED: &[u8] = b"treasury";

pub const PROTOCOL_FEE_BPS: u16 = 1500;
pub const BPS_DENOMINATOR: u64 = 10_000;

pub const CLAIM_WINDOW_SECONDS: i64 = 7 * 24 * 60 * 60;
pub const LIQUIDATION_GRACE_SECONDS: i64 = 14 * 24 * 60 * 60;

pub const VIRTUAL_ASSETS_OFFSET: u64 = 1_000_000;
pub const VIRTUAL_SHARES_OFFSET: u64 = 1_000_000_000;

#[account]
pub struct Pool {
    pub bump: u8,
    pub asset_mint: Pubkey,
    pub usdc_vault: Pubkey,
    pub share_mint: Pubkey,
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
    pub paused: bool,
}

impl Pool {
    pub const SIZE: usize = 8
        + 1
        + 32 * 4
        + 8 * 4
        + FeeCurve::SIZE
        + 2
        + 8
        + 8
        + 1 + PendingParams::SIZE
        + 1
        + 64;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Default)]
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
    pub const SIZE: usize = 2 * 7;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PendingParams {
    pub fee_curve: FeeCurve,
    pub max_advance_pct_bps: u16,
    pub max_advance_abs: u64,
    pub execute_after: i64,
}

impl PendingParams {
    pub const SIZE: usize = FeeCurve::SIZE + 2 + 8 + 8;
}

#[account]
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

impl Advance {
    pub const SIZE: usize = 8
        + 1
        + 32
        + 32
        + 8 * 5
        + 1
        + 1 + 32
        + 1
        + 32;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
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

#[account]
pub struct ConsumedPayment {
    pub bump: u8,
    pub nonce: [u8; 16],
    pub agent: Pubkey,
    pub created_at: i64,
}

impl ConsumedPayment {
    pub const SIZE: usize = 8 + 1 + 16 + 32 + 8 + 16;
}

#[account]
pub struct ProtocolTreasury {
    pub bump: u8,
    pub authority: Pubkey,
    pub treasury_ata: Pubkey,
    pub total_collected: u64,
}

impl ProtocolTreasury {
    pub const SIZE: usize = 8 + 1 + 32 + 32 + 8 + 32;
}
