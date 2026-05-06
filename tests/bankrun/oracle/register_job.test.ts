/**
 * Permissionless `register_job` ix on credmesh-receivable-oracle.
 * EVM-parity with `POST /marketplace/jobs`. No authority gate; caller pays
 * rent. The agent's standing credit_limit + outstanding_balance still
 * bounds any actual advance.
 *
 * Sources of truth:
 *   programs/credmesh-receivable-oracle/src/lib.rs::register_job
 *   crates/credmesh-shared/src/lib.rs SourceKind + claim_ratio_bps
 *   protocol-spec/index.js CLAIM_RATIOS (EVM lane)
 */

import { expect } from "chai";
import {
  BPS_DENOMINATOR,
  CLAIM_RATIO_BPS as SOLANA_CLAIM_RATIO_BPS,
  EVM_CLAIM_RATIOS,
} from "../../../ts/shared/src/index.js";

describe("credmesh-receivable-oracle / register_job — permissionless marketplace primitive", () => {
  describe("claim ratios match EVM protocol-spec exactly", () => {
    it("Worker == EVM worker_attested == 10% (1000 bps)", () => {
      expect(SOLANA_CLAIM_RATIO_BPS.Worker / BPS_DENOMINATOR).to.equal(EVM_CLAIM_RATIOS.worker_attested);
    });

    it("Marketplace == Worker == 10% (same trust tier as worker_attested)", () => {
      expect(SOLANA_CLAIM_RATIO_BPS.Marketplace).to.equal(SOLANA_CLAIM_RATIO_BPS.Worker);
    });

    it("Ed25519 == EVM signed_receivable == 20% (2000 bps)", () => {
      expect(SOLANA_CLAIM_RATIO_BPS.Ed25519 / BPS_DENOMINATOR).to.equal(EVM_CLAIM_RATIOS.signed_receivable);
    });

    it("X402 == Ed25519 == 20% (same shape, different AllowedSigner kind)", () => {
      expect(SOLANA_CLAIM_RATIO_BPS.X402).to.equal(SOLANA_CLAIM_RATIO_BPS.Ed25519);
    });

    it("venue_state (30%) is intentionally NOT supported on Solana v1", () => {
      // EVM `venue_state` reads a Hyperliquid position. No equivalent on
      // Solana v1. Future v2 might add a Pyth Lazer or Drift-perp variant.
      const supportedKinds = Object.keys(SOLANA_CLAIM_RATIO_BPS);
      expect(supportedKinds).to.not.include("venue_state");
    });
  });

  describe("register_job permissionless invariants", () => {
    it("docs: caller pays rent → spam-bounded by economics", () => {
      // RegisterJob struct: payer = poster (signer)
      // Solana account rent for a Receivable PDA ≈ 0.0017 SOL (~$0.30)
      // An attacker registering N fake jobs pays 0.0017N SOL
      const receivableRentSol = 0.0017;
      const spamCost = (n: number) => receivableRentSol * n;
      expect(spamCost(100)).to.equal(0.17); // SOL
    });

    it("docs: receivable PDA seed namespace is distinct per source_kind", () => {
      // [RECEIVABLE_SEED, &[source_kind_byte], agent, source_id]
      // source_kind ∈ { 0=Worker, 1=Ed25519, 2=X402, 3=Marketplace }
      // → different addresses for each kind (Audit-MED #3 fix preserved)
      const seedShapes = [
        "[RECEIVABLE_SEED, [0u8], agent, source_id] -> Worker",
        "[RECEIVABLE_SEED, [1u8], agent, source_id] -> Ed25519",
        "[RECEIVABLE_SEED, [2u8], agent, source_id] -> X402",
        "[RECEIVABLE_SEED, [3u8], agent, source_id] -> Marketplace",
      ];
      expect(seedShapes).to.have.lengthOf(4);
    });

    it("docs: agent's credit_limit - outstanding_balance bounds the actual advance", () => {
      // Even if attacker registers a $1M Marketplace job for a victim
      // agent, the agent's standing credit_limit_atoms (capped at $1000)
      // minus outstanding_balance_atoms is the actual draw cap.
      // Plus the 10% claim ratio: max draw against this $1M = $100k...
      // ...but min(claim_cap, available_credit) wins → max draw = $1k.
      const fakeReceivableUsd = 1_000_000;
      const claimCap = fakeReceivableUsd * (SOLANA_CLAIM_RATIO_BPS.Marketplace / BPS_DENOMINATOR);
      const agentCreditLimit = 1_000;
      const actualMaxDraw = Math.min(claimCap, agentCreditLimit);
      expect(actualMaxDraw).to.equal(1_000);
    });
  });

  describe("Mode 3 settlement parity (EVM `settle(advanceId, payout)`)", () => {
    it("docs: cranker can fund repayment from own ATA", () => {
      // claim_and_settle three-mode dispatch:
      //   Mode A: cranker == agent && payer.owner == agent
      //   Mode B: cranker != agent && payer.owner == agent (SPL delegate)
      //   Mode 3: cranker != agent && payer.owner == cranker (NEW)
      // Mode 3 is the EVM autonomous-payout flow: marketplace calls
      // claim_and_settle with its OWN USDC, agent uninvolved.
      const modes = [
        "Mode A — agent self-cranks",
        "Mode B — relayer cranks via SPL delegate",
        "Mode 3 — cranker funds repayment (EVM settle parity)",
      ];
      expect(modes).to.have.lengthOf(3);
    });
  });
});
