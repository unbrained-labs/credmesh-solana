use anchor_lang::prelude::*;

use crate::errors::CredmeshError;
use crate::state::{
    FeeCurve, Pool, BPS_DENOMINATOR, VIRTUAL_ASSETS_OFFSET, VIRTUAL_SHARES_OFFSET,
};

/// Virtual-shares math (OZ ERC-4626 `_decimalsOffset` pattern, ported to u128
/// to avoid intermediate overflow). With the offsets set in `state.rs`, a 1-atom
/// inflation attack costs ≥10⁶× any extractable profit.
///
/// shares_minted = (amount * (total_shares + V_S)) / (total_assets + V_A)
#[inline]
pub fn preview_deposit(amount: u64, total_assets: u64, total_shares: u64) -> Result<u64> {
    let amount_u = amount as u128;
    let shares_off = (total_shares as u128)
        .checked_add(VIRTUAL_SHARES_OFFSET as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    let assets_off = (total_assets as u128)
        .checked_add(VIRTUAL_ASSETS_OFFSET as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    let numerator = amount_u
        .checked_mul(shares_off)
        .ok_or(CredmeshError::MathOverflow)?;
    let shares = numerator
        .checked_div(assets_off)
        .ok_or(CredmeshError::MathOverflow)?;
    u64::try_from(shares).map_err(|_| error!(CredmeshError::MathOverflow))
}

/// assets_returned = (shares * (total_assets + V_A)) / (total_shares + V_S)
#[inline]
pub fn preview_redeem(shares: u64, total_assets: u64, total_shares: u64) -> Result<u64> {
    let shares_u = shares as u128;
    let assets_off = (total_assets as u128)
        .checked_add(VIRTUAL_ASSETS_OFFSET as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    let shares_off = (total_shares as u128)
        .checked_add(VIRTUAL_SHARES_OFFSET as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    let numerator = shares_u
        .checked_mul(assets_off)
        .ok_or(CredmeshError::MathOverflow)?;
    let assets = numerator
        .checked_div(shares_off)
        .ok_or(CredmeshError::MathOverflow)?;
    u64::try_from(assets).map_err(|_| error!(CredmeshError::MathOverflow))
}

/// Compute the per-issuance fee. Mirrors `pricing.ts` shape:
/// utilization premium + duration premium + risk premium + (pool loss surcharge omitted in v1).
/// Returns USDC atoms (6 decimals).
#[inline]
pub fn compute_fee_amount(
    principal: u64,
    duration_seconds: u64,
    utilization_bps: u64,
    default_count: u32,
    curve: &FeeCurve,
) -> Result<u64> {
    let mut rate_bps: u64 = curve.base_rate_bps as u64;

    // Utilization kink (linear above kink → max).
    let kink = curve.utilization_kink_bps as u64;
    if utilization_bps > kink && (BPS_DENOMINATOR.saturating_sub(kink)) > 0 {
        let extra = utilization_bps - kink;
        let span = BPS_DENOMINATOR - kink;
        let kink_to_max = (curve.max_rate_bps as u64).saturating_sub(curve.kink_rate_bps as u64);
        rate_bps = curve.kink_rate_bps as u64
            + extra
                .checked_mul(kink_to_max)
                .ok_or(CredmeshError::MathOverflow)?
                / span;
    } else {
        let kink_minus_base =
            (curve.kink_rate_bps as u64).saturating_sub(curve.base_rate_bps as u64);
        let scaled = utilization_bps
            .checked_mul(kink_minus_base)
            .ok_or(CredmeshError::MathOverflow)?;
        rate_bps += if kink > 0 { scaled / kink } else { 0 };
    }

    // Duration premium.
    let duration_days = duration_seconds / 86_400;
    rate_bps = rate_bps
        .checked_add(duration_days.saturating_mul(curve.duration_per_day_bps as u64))
        .ok_or(CredmeshError::MathOverflow)?;

    // Risk premium scales with default_count (clamped at 5).
    let risk_factor = (default_count as u64).min(5);
    rate_bps = rate_bps
        .checked_add(risk_factor.saturating_mul(curve.risk_premium_bps as u64))
        .ok_or(CredmeshError::MathOverflow)?;

    rate_bps = rate_bps.min(curve.max_rate_bps as u64);

    let fee_u128 = (principal as u128)
        .checked_mul(rate_bps as u128)
        .ok_or(CredmeshError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    u64::try_from(fee_u128).map_err(|_| error!(CredmeshError::MathOverflow))
}

#[inline]
pub fn compute_late_penalty_per_day(principal: u64, curve: &FeeCurve) -> Result<u64> {
    // 0.1% per day of principal, multiplied by pool_loss_surcharge_bps if active.
    let base = (principal as u128)
        .checked_mul(10) // 0.1% = 10 bps
        .ok_or(CredmeshError::MathOverflow)?
        .checked_div(BPS_DENOMINATOR as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    let surcharge = curve.pool_loss_surcharge_bps as u128;
    let total = if surcharge > 0 {
        base.checked_mul(BPS_DENOMINATOR as u128 + surcharge)
            .ok_or(CredmeshError::MathOverflow)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(CredmeshError::MathOverflow)?
    } else {
        base
    };
    u64::try_from(total).map_err(|_| error!(CredmeshError::MathOverflow))
}

#[inline]
pub fn compute_utilization_bps(pool: &Pool) -> Result<u64> {
    if pool.total_assets == 0 {
        return Ok(BPS_DENOMINATOR);
    }
    let utilization_u128 = (pool.deployed_amount as u128)
        .checked_mul(BPS_DENOMINATOR as u128)
        .ok_or(CredmeshError::MathOverflow)?
        .checked_div(pool.total_assets as u128)
        .ok_or(CredmeshError::MathOverflow)?;
    Ok(u64::try_from(utilization_u128).unwrap_or(BPS_DENOMINATOR))
}
