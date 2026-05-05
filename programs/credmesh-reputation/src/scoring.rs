//! Credit-score and credit-limit math, ported from the EVM lane's
//! `packages/credit-worker/src/credit.ts:24-56`. Tests assert the on-chain
//! and off-chain numbers match for the reference vectors.
//!
//! All inputs and outputs are integer USDC atoms (6 decimals). The EVM lane
//! uses USD floats; the conversion factor 1e6 is folded into the constants
//! so the relative weights match exactly.

use crate::state::{AgentReputation, MAX_CREDIT_LIMIT_ATOMS, SCORE_AVG_PAYOUT_CAP_ATOMS, SCORE_OUTSTANDING_CAP_ATOMS};

/// Compute credit score 0..100. Mirrors `credit.ts::computeCreditProfile`
/// score block:
///   identityRegistered ? 10 : 0
///   + min(repaidAdvances, 10) * 5
///   + min(successfulJobs, 20) * 1.6
///   + min(attestationCount, 20) * 0.7
///   + min(cooperationSuccessCount, 10) * 1.5
///   + trustScore * 0.08
///   + min(averageCompletedPayout, 200) * 0.02
///   - failedJobs * 6
///   - defaultedAdvances * 25
///   - min(outstandingBalance, 100) * 0.2
///
/// Multiplied by 100 internally to keep the fractional weights as integer
/// divisions; final result is divided back down.
pub fn compute_credit_score(rep: &AgentReputation) -> u32 {
    let mut score_x100: i64 = 0;

    if rep.identity_registered {
        score_x100 += 10 * 100;
    }
    score_x100 += (rep.repaid_advances.min(10) as i64) * 500;
    score_x100 += (rep.successful_jobs.min(20) as i64) * 160;
    score_x100 += (rep.attestation_count.min(20) as i64) * 70;
    score_x100 += (rep.cooperation_success_count.min(10) as i64) * 150;
    score_x100 += (rep.trust_score.min(100) as i64) * 8;
    // average_completed_payout in atoms; divide by 1e6 to dollars before *0.02.
    let avg_payout_clamped = rep.average_completed_payout_atoms.min(SCORE_AVG_PAYOUT_CAP_ATOMS);
    score_x100 += (avg_payout_clamped as i64) * 2 / 1_000_000;

    score_x100 -= (rep.failed_jobs as i64) * 600;
    score_x100 -= (rep.defaulted_advances as i64) * 2500;
    let outstanding_clamped = rep.outstanding_balance_atoms.min(SCORE_OUTSTANDING_CAP_ATOMS);
    score_x100 -= (outstanding_clamped as i64) * 20 / 1_000_000;

    let score = (score_x100 + 50) / 100; // round-half-up
    score.clamp(0, 100) as u32
}

/// Compute standing credit limit in USDC atoms. Mirrors
/// `credit.ts::computeCreditProfile`:
///   rawLimit = creditScore * 8 + repayRate * 120 + completionRate * 80
///   creditLimit = clamp(rawLimit, 0, 1000)
/// where repayRate and completionRate are 0..1 floats (atoms-equivalent
/// here: 0..1_000_000).
///
/// Returned in USDC atoms (clamped to `MAX_CREDIT_LIMIT_ATOMS`).
pub fn compute_credit_limit_atoms(rep: &AgentReputation) -> u64 {
    let credit_score = rep.credit_score as u64;

    let total_advances = rep.repaid_advances as u64 + rep.defaulted_advances as u64;
    // repay_rate as atoms (0..1_000_000): repaid / total
    let repay_rate_atoms: u64 = if total_advances == 0 {
        0
    } else {
        (rep.repaid_advances as u64).saturating_mul(1_000_000) / total_advances
    };

    let total_jobs = rep.successful_jobs as u64 + rep.failed_jobs as u64;
    // EVM bootstrap: completionRate defaults to 0.5 when no jobs.
    let completion_rate_atoms: u64 = if total_jobs == 0 {
        500_000
    } else {
        (rep.successful_jobs as u64).saturating_mul(1_000_000) / total_jobs
    };

    // raw_limit_dollars = credit_score * 8 + repay_rate * 120 + completion_rate * 80
    // Convert to atoms by multiplying dollar terms by 1_000_000:
    //   credit_score * 8 * 1_000_000
    //   repay_rate_atoms * 120     (repay_rate is already scaled by 1_000_000)
    //   completion_rate_atoms * 80
    let raw_atoms = credit_score
        .saturating_mul(8 * 1_000_000)
        .saturating_add(repay_rate_atoms.saturating_mul(120))
        .saturating_add(completion_rate_atoms.saturating_mul(80));

    raw_atoms.min(MAX_CREDIT_LIMIT_ATOMS)
}

/// Available credit = credit_limit - outstanding_balance, saturating at 0.
pub fn compute_available_credit_atoms(rep: &AgentReputation) -> u64 {
    rep.credit_limit_atoms
        .saturating_sub(rep.outstanding_balance_atoms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_agent() -> AgentReputation {
        AgentReputation {
            bump: 0,
            agent: anchor_lang::prelude::Pubkey::default(),
            credit_score: 0,
            credit_limit_atoms: 0,
            outstanding_balance_atoms: 0,
            trust_score: 0,
            attestation_count: 0,
            cooperation_success_count: 0,
            successful_jobs: 0,
            failed_jobs: 0,
            repaid_advances: 0,
            defaulted_advances: 0,
            average_completed_payout_atoms: 0,
            identity_registered: false,
            feedback_count: 0,
            feedback_digest: [0u8; 32],
            score_ema: 0,
            default_count: 0,
            last_event_slot: 0,
        }
    }

    #[test]
    fn fresh_agent_score_is_zero() {
        let rep = empty_agent();
        assert_eq!(compute_credit_score(&rep), 0);
    }

    #[test]
    fn fresh_agent_with_identity_gets_floor_score_10() {
        let mut rep = empty_agent();
        rep.identity_registered = true;
        assert_eq!(compute_credit_score(&rep), 10);
    }

    #[test]
    fn evm_reference_buildbot_score() {
        // EVM README "BuildBot" sample input → expected score around 78.
        // identityRegistered=false, trustScore=78, attestationCount=9,
        // cooperationSuccessCount=6, successfulJobs=8, failedJobs=1,
        // averageCompletedPayout=92.
        let mut rep = empty_agent();
        rep.trust_score = 78;
        rep.attestation_count = 9;
        rep.cooperation_success_count = 6;
        rep.successful_jobs = 8;
        rep.failed_jobs = 1;
        rep.average_completed_payout_atoms = 92 * 1_000_000;
        // EVM hand-calc (per credit.ts):
        // 0 (identity) + 0*5 (repaid) + min(8,20)*1.6 + min(9,20)*0.7
        //   + min(6,10)*1.5 + 78*0.08 + min(92,200)*0.02
        //   - 1*6 - 0*25 - 0*0.2
        // = 0 + 0 + 12.8 + 6.3 + 9.0 + 6.24 + 1.84 - 6 = 30.18 → rounds to 30
        let s = compute_credit_score(&rep);
        assert!(s >= 29 && s <= 31, "expected ~30, got {}", s);
    }

    #[test]
    fn credit_limit_caps_at_1000_dollars() {
        let mut rep = empty_agent();
        rep.credit_score = 100;
        rep.repaid_advances = 100;
        rep.successful_jobs = 100;
        // raw_limit = 100*8 + 1.0*120 + 1.0*80 = 1000 → capped at 1000.
        assert_eq!(compute_credit_limit_atoms(&rep), MAX_CREDIT_LIMIT_ATOMS);
    }

    #[test]
    fn credit_limit_score_50_perfect_history() {
        let mut rep = empty_agent();
        rep.credit_score = 50;
        rep.repaid_advances = 5;
        rep.defaulted_advances = 0;
        rep.successful_jobs = 10;
        rep.failed_jobs = 0;
        // raw_limit = 50*8 + 1.0*120 + 1.0*80 = 600 dollars
        assert_eq!(compute_credit_limit_atoms(&rep), 600 * 1_000_000);
    }

    #[test]
    fn credit_limit_no_history_uses_completion_default_half() {
        let mut rep = empty_agent();
        rep.credit_score = 50;
        // no advances, no jobs → repay_rate 0, completion_rate 0.5
        // raw_limit = 50*8 + 0*120 + 0.5*80 = 400 + 0 + 40 = 440 dollars
        assert_eq!(compute_credit_limit_atoms(&rep), 440 * 1_000_000);
    }

    #[test]
    fn available_credit_is_limit_minus_outstanding() {
        let mut rep = empty_agent();
        rep.credit_limit_atoms = 500_000_000; // $500
        rep.outstanding_balance_atoms = 200_000_000; // $200
        assert_eq!(compute_available_credit_atoms(&rep), 300_000_000);
    }

    #[test]
    fn available_credit_saturates_at_zero() {
        let mut rep = empty_agent();
        rep.credit_limit_atoms = 100_000_000;
        rep.outstanding_balance_atoms = 200_000_000;
        assert_eq!(compute_available_credit_atoms(&rep), 0);
    }
}
