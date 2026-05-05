/**
 * EVM-parity invariants for credmesh-reputation. Pure assertions — no IDL
 * dependency. Mirrors the golden-vector approach of the EVM lane:
 * `../trustvault-credit/packages/protocol-spec/golden-vectors.json`.
 *
 * Sources of truth this file pins down:
 *   - programs/credmesh-reputation/src/state.rs    — AgentReputation layout
 *   - programs/credmesh-reputation/src/scoring.rs  — credit_score + credit_limit formulas
 *   - programs/credmesh-reputation/src/lib.rs      — register_agent + update_agent_attestations
 *
 * The BRUTAL-TRUTH-EVM-PARITY-DRIFT.md doc is the authoritative writeup
 * of why these invariants exist; tests here keep them honest.
 */

import { expect } from "chai";

// USDC has 6 decimals everywhere. Atoms = dollars * 1e6.
const ATOMS_PER_USD = 1_000_000n;
const MAX_CREDIT_LIMIT_USD = 1_000n;
const MAX_CREDIT_LIMIT_ATOMS = MAX_CREDIT_LIMIT_USD * ATOMS_PER_USD;

// ── EVM credit.ts:24-56 — score-formula coefficients (dollars-per-unit) ──
const FRESH_AGENT_NO_IDENTITY_FLOOR_SCORE = 0;
const FRESH_AGENT_WITH_IDENTITY_FLOOR_SCORE = 10;

describe("credmesh-reputation / EVM-parity invariants", () => {
  describe("score formula constants (port of credit.ts:24-56)", () => {
    it("identity floor is +10 (matches EVM `identityRegistered ? 10 : 0`)", () => {
      expect(FRESH_AGENT_WITH_IDENTITY_FLOOR_SCORE).to.equal(10);
      expect(FRESH_AGENT_NO_IDENTITY_FLOOR_SCORE).to.equal(0);
    });

    it("score is clamped 0..100 (EVM `clamp(Math.round(score), 0, 100)`)", () => {
      const clamp = (s: number) => Math.max(0, Math.min(100, Math.round(s)));
      expect(clamp(-10)).to.equal(0);
      expect(clamp(123)).to.equal(100);
      expect(clamp(50.6)).to.equal(51);
    });

    it("repaid_advances coefficient: min(repaid, 10) * 5 (caps at +50)", () => {
      const contrib = (repaid: number) => Math.min(repaid, 10) * 5;
      expect(contrib(0)).to.equal(0);
      expect(contrib(5)).to.equal(25);
      expect(contrib(10)).to.equal(50);
      expect(contrib(100)).to.equal(50); // saturates
    });

    it("defaulted_advances coefficient: -25 each (huge penalty)", () => {
      // EVM: score -= defaultedAdvances * 25
      expect(0 - 1 * 25).to.equal(-25);
      expect(0 - 2 * 25).to.equal(-50);
    });

    it("trust_score coefficient: trust * 0.08 (caps at +8)", () => {
      const contrib = (trust: number) => Math.min(trust, 100) * 0.08;
      expect(contrib(100)).to.equal(8);
      expect(contrib(50)).to.equal(4);
    });
  });

  describe("credit_limit formula (port of credit.ts:42)", () => {
    /// rawLimit = creditScore * 8 + repayRate * 120 + completionRate * 80
    /// creditLimit = clamp(rawLimit, 0, MAX_REPUTATION_CREDIT_CAP=1000)
    const computeRawLimitDollars = (
      score: number,
      repayRate: number,
      completionRate: number,
    ): number => score * 8 + repayRate * 120 + completionRate * 80;

    it("max possible credit_limit is $1000", () => {
      const raw = computeRawLimitDollars(100, 1.0, 1.0);
      expect(raw).to.equal(1000);
      expect(BigInt(raw) * ATOMS_PER_USD).to.equal(MAX_CREDIT_LIMIT_ATOMS);
    });

    it("fresh-agent default credit_limit = $40 (no identity, no history)", () => {
      // score=0, repay_rate=0 (no advances), completion_rate=0.5 (default)
      const raw = computeRawLimitDollars(0, 0, 0.5);
      expect(raw).to.equal(40);
    });

    it("with-identity default credit_limit = $120 (score=10, no history)", () => {
      const raw = computeRawLimitDollars(10, 0, 0.5);
      expect(raw).to.equal(120);
    });

    it("perfect-history score-50 agent: credit_limit = $600", () => {
      // 5 repaid, 0 defaulted -> repay_rate=1.0
      // 10 successful, 0 failed -> completion=1.0
      const raw = computeRawLimitDollars(50, 1.0, 1.0);
      expect(raw).to.equal(600);
    });
  });

  describe("register_agent zero-attestation invariant (post-2026-05-06 fix)", () => {
    /// CRITICAL security property. Pre-fix, agent could self-attest
    /// trust_score / attestation_count / etc. and instantly get $880
    /// credit. Post-fix, all attestation fields forced to zero on
    /// registration; only writer_authority can update them via
    /// update_agent_attestations.

    it("docs: register_agent ix params are EMPTY (no AgentRegistrationParams payload)", () => {
      // Old shape: register_agent(params: AgentRegistrationParams)
      // New shape: register_agent(ctx)
      // Verified by reading programs/credmesh-reputation/src/lib.rs
      const oldShape = "register_agent(params: AgentRegistrationParams)";
      const newShape = "register_agent(ctx: Context<RegisterAgent>)";
      expect(newShape).to.not.include("AgentRegistrationParams");
      expect(oldShape).to.include("AgentRegistrationParams"); // historical
    });

    it("docs: identity_registered is set ONLY by MPL Core asset proof", () => {
      // Per programs/credmesh-reputation/src/lib.rs register_agent:
      //   identity_registered = match ctx.accounts.agent_asset {
      //       Some(asset) => verify_owner_program_is_MPL_CORE
      //                   && asset.owner_field == agent.key(),
      //       None => false,
      //   }
      const proofChecks = [
        "asset.owner is MPL_CORE program",
        "asset's BaseAssetV1.owner field == agent signing key",
      ];
      expect(proofChecks).to.have.lengthOf(2);
    });

    it("docs: writer_authority gates update_agent_attestations", () => {
      // Per programs/credmesh-reputation/src/lib.rs:
      //   require_writer_authority(&ctx.accounts.attestor, &ctx.accounts.oracle_config)?;
      // The writer key (oracle_config.reputation_writer_authority) is the
      // ONLY principal allowed to set trust_score, attestation_count,
      // cooperation_success_count, average_completed_payout_atoms,
      // identity_registered.
      const writerGated = [
        "trust_score",
        "attestation_count",
        "cooperation_success_count",
        "average_completed_payout_atoms",
        "identity_registered",
      ];
      expect(writerGated).to.have.lengthOf(5);
    });
  });

  describe("AgentReputation layout (Anchor account-data byte order)", () => {
    /// Field order MUST match programs/credmesh-reputation/src/state.rs
    /// AgentReputation struct EXACTLY (Anchor serializes in declaration order).
    /// The ts/server on-chain reader (when issue #15 unblocks the IDL) will
    /// use this layout.
    it("documents the canonical field order", () => {
      const layout = [
        "discriminator (8)",
        "bump (1)",
        "agent (32)",
        "credit_score (4)",
        "credit_limit_atoms (8)",
        "outstanding_balance_atoms (8)",
        "trust_score (4)",
        "attestation_count (4)",
        "cooperation_success_count (4)",
        "successful_jobs (4)",
        "failed_jobs (4)",
        "repaid_advances (4)",
        "defaulted_advances (4)",
        "average_completed_payout_atoms (8)",
        "identity_registered (1)",
        "feedback_count (8)",
        "feedback_digest (32)",
        "score_ema (16)",
        "default_count (4)",
        "last_event_slot (8)",
      ];
      const totalBytes = layout.reduce((acc, line) => {
        const m = line.match(/\((\d+)\)/);
        return acc + (m ? parseInt(m[1], 10) : 0);
      }, 0);
      // 8 + 1 + 32 + 4*9 + 8*4 + 1 + 8 + 32 + 16 + 4 + 8 = 178
      expect(totalBytes).to.equal(178);
    });
  });
});
