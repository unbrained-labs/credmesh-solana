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

#[cfg(test)]
mod tests {
    //! Pure-math tests for the pricing surface. Run with
    //! `cargo test -p credmesh-escrow --lib pricing::tests`.
    //!
    //! These tests are deterministic, off-chain, and do NOT require a
    //! validator — they exercise just the `pricing.rs` arithmetic.
    //! Catches regressions on every push without the bankrun /
    //! anchor-build / IDL pipeline.
    use super::*;

    fn curve() -> FeeCurve {
        // Representative bounds (EVM-parity-shaped). All invariants
        // satisfy `FeeCurve::validate()`.
        FeeCurve {
            utilization_kink_bps: 8_000,
            base_rate_bps: 200,
            kink_rate_bps: 800,
            max_rate_bps: 5_000,
            duration_per_day_bps: 50,
            risk_premium_bps: 100,
            pool_loss_surcharge_bps: 0,
        }
    }

    // ── compute_fee_amount ──────────────────────────────────────────────────

    #[test]
    fn fee_zero_principal_is_zero() {
        let f = compute_fee_amount(0, 86_400, 5_000, 0, &curve()).unwrap();
        assert_eq!(f, 0);
    }

    #[test]
    fn fee_capped_by_max_rate_bps() {
        // Force every premium high; assert fee never exceeds
        // principal * max_rate_bps / BPS_DENOMINATOR.
        let principal: u64 = 1_000_000_000; // $1000
        let c = curve();
        let f = compute_fee_amount(principal, 86_400 * 365, 10_000, 99, &c).unwrap();
        let upper = (principal as u128) * (c.max_rate_bps as u128) / (BPS_DENOMINATOR as u128);
        assert!(
            (f as u128) <= upper,
            "fee {} exceeded ceiling {}",
            f,
            upper
        );
    }

    #[test]
    fn fee_monotonic_in_principal() {
        let c = curve();
        let a = compute_fee_amount(10_000_000, 86_400, 5_000, 0, &c).unwrap();
        let b = compute_fee_amount(20_000_000, 86_400, 5_000, 0, &c).unwrap();
        let cv = compute_fee_amount(50_000_000, 86_400, 5_000, 0, &c).unwrap();
        assert!(a <= b);
        assert!(b <= cv);
    }

    #[test]
    fn fee_monotonic_in_duration() {
        let c = curve();
        let p = 100_000_000;
        let one_day = compute_fee_amount(p, 86_400, 5_000, 0, &c).unwrap();
        let seven_day = compute_fee_amount(p, 7 * 86_400, 5_000, 0, &c).unwrap();
        let thirty_day = compute_fee_amount(p, 30 * 86_400, 5_000, 0, &c).unwrap();
        assert!(one_day <= seven_day);
        assert!(seven_day <= thirty_day);
    }

    #[test]
    fn fee_monotonic_in_utilization_above_kink() {
        let c = curve();
        let p = 100_000_000;
        let at_kink = compute_fee_amount(p, 86_400, c.utilization_kink_bps as u64, 0, &c).unwrap();
        let above = compute_fee_amount(p, 86_400, 9_500, 0, &c).unwrap();
        let max_util = compute_fee_amount(p, 86_400, 10_000, 0, &c).unwrap();
        assert!(at_kink <= above);
        assert!(above <= max_util);
    }

    #[test]
    fn fee_risk_premium_clamps_at_five_defaults() {
        let c = curve();
        let p = 100_000_000;
        let five = compute_fee_amount(p, 86_400, 5_000, 5, &c).unwrap();
        let twenty = compute_fee_amount(p, 86_400, 5_000, 20, &c).unwrap();
        // 5+ defaults must produce identical fees — clamping prevents
        // griefing scenarios where an attacker drives default_count
        // arbitrarily high to inflate fees.
        assert_eq!(five, twenty);
    }

    // ── compute_late_penalty_per_day ────────────────────────────────────────

    #[test]
    fn late_penalty_zero_principal_zero() {
        assert_eq!(compute_late_penalty_per_day(0, &curve()).unwrap(), 0);
    }

    #[test]
    fn late_penalty_baseline_is_ten_bps() {
        // 0.1% of principal per day = 10 bps; surcharge=0.
        let p: u64 = 1_000_000_000; // $1000
        let pen = compute_late_penalty_per_day(p, &curve()).unwrap();
        assert_eq!(pen, p * 10 / 10_000); // 1_000_000 = $1
    }

    #[test]
    fn late_penalty_surcharge_increases() {
        let p: u64 = 100_000_000;
        let no_surcharge = compute_late_penalty_per_day(p, &curve()).unwrap();
        let mut c2 = curve();
        c2.pool_loss_surcharge_bps = 5_000; // 50% surcharge
        let with_surcharge = compute_late_penalty_per_day(p, &c2).unwrap();
        assert!(with_surcharge > no_surcharge);
        // At 50% surcharge: penalty * 1.5
        assert_eq!(with_surcharge, no_surcharge * 3 / 2);
    }

    // ── virtual-shares / preview math ───────────────────────────────────────

    #[test]
    fn first_depositor_inflation_attack_is_costly() {
        // Attacker deposits 1 atom, transfers 1_000_000_000 USDC directly
        // to the vault to inflate share price, then a victim deposits.
        // With virtual offsets, the victim must still receive non-trivial
        // shares (i.e., the inflation attack barely moves the share
        // price). Concretely: a 1-atom share holder cannot extract more
        // than 1 atom on redeem.
        let attacker_shares = preview_deposit(1, 0, 0).unwrap();
        // Attacker pumps assets without minting shares (direct transfer).
        let total_assets = 1_000_000_000_000u64; // $1M
        let attacker_redeem = preview_redeem(attacker_shares, total_assets, attacker_shares).unwrap();
        // The attacker's 1-atom contribution should NOT extract anywhere
        // close to the inflated $1M because of the virtual-shares offset.
        // Expect: redeem << total_assets.
        assert!(
            attacker_redeem < total_assets / 100,
            "attacker recovered {}/{} (≥1%); virtual offsets too small",
            attacker_redeem,
            total_assets
        );
    }

    #[test]
    fn share_price_monotonic_under_yield_addition() {
        // Sequence: deposit 100 → vault accrues 50 in yield → second
        // deposit of 100 must mint FEWER shares than the first (because
        // share price rose). Monotonicity invariant.
        let s1 = preview_deposit(100_000_000, 0, 0).unwrap();
        // After yield, total_assets grew but total_shares stayed flat.
        let s2 = preview_deposit(100_000_000, 100_000_000 + 50_000_000, s1).unwrap();
        assert!(s2 < s1, "second deposit minted {} >= first {}", s2, s1);
    }

    #[test]
    fn redeem_round_trips_with_no_yield() {
        // No yield, no other depositors: redeem should return ~ what was
        // deposited (some rounding loss permitted by virtual offsets).
        let dep = 1_000_000_000u64;
        let shares = preview_deposit(dep, 0, 0).unwrap();
        let redeemed = preview_redeem(shares, dep, shares).unwrap();
        // Allow small rounding loss (≤ 1 atom per million on round-trip).
        let loss = dep.saturating_sub(redeemed);
        assert!(loss <= dep / 1_000_000, "round-trip loss {}/{}", loss, dep);
    }

    // ── compute_utilization_bps ─────────────────────────────────────────────

    fn pool_with(total_assets: u64, deployed: u64) -> Pool {
        Pool {
            bump: 0,
            asset_mint: Pubkey::default(),
            usdc_vault: Pubkey::default(),
            share_mint: Pubkey::default(),
            treasury_ata: Pubkey::default(),
            governance: Pubkey::default(),
            total_assets,
            total_shares: 0,
            deployed_amount: deployed,
            accrued_protocol_fees: 0,
            fee_curve: curve(),
            max_advance_pct_bps: 3000,
            max_advance_abs: 100_000_000,
            agent_window_cap: 0,
            timelock_seconds: 86_400,
            chain_id: 2,
            pending_params: None,
        }
    }

    #[test]
    fn utilization_zero_assets_is_max() {
        let p = pool_with(0, 0);
        assert_eq!(compute_utilization_bps(&p).unwrap(), BPS_DENOMINATOR);
    }

    #[test]
    fn utilization_half_deployed_is_5000_bps() {
        let p = pool_with(1_000_000_000, 500_000_000);
        assert_eq!(compute_utilization_bps(&p).unwrap(), 5_000);
    }

    #[test]
    fn utilization_fully_deployed_is_max() {
        let p = pool_with(1_000_000_000, 1_000_000_000);
        assert_eq!(compute_utilization_bps(&p).unwrap(), BPS_DENOMINATOR);
    }
}
