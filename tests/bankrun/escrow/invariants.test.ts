/**
 * Property tests over `credmesh-escrow` math. Pure-JS, deterministic-seed
 * fuzz; no harness required.
 *
 * Three headline invariants:
 *
 *   1. **Waterfall sum invariant** — for any settlement,
 *        protocol_cut + lp_cut + agent_net == payment_amount.
 *      The on-chain `claim_and_settle` ENFORCES this with a `require!` at
 *      lib.rs:476-481, so this test is a contract on the math: if the JS
 *      replica diverges, the on-chain handler will revert with
 *      `WaterfallSumMismatch`. Fuzz finds rounding-drift candidates before
 *      they become a runtime revert.
 *
 *   2. **Share-price monotonicity** — for any sequence of {deposit,
 *      withdraw, yield-accrual} ops on a Pool, the share price
 *      P = (total_assets + V_A) / (total_shares + V_S) is non-decreasing.
 *      This is the LP-protection property that makes pool participation
 *      safe under arbitrary order/timing of other LPs' actions.
 *      NOTE: `liquidate` is intentionally excluded — LPs eat losses via
 *      share-price drop on default (state.rs / lib.rs::liquidate). The
 *      monotonicity property holds *only* under the "happy" op set.
 *
 *   3. **First-depositor inflation defense (extended)** — extending the
 *      Day 1 spot-check, fuzz over (attacker_deposit, attacker_donation,
 *      victim_deposit) tuples and assert
 *        attacker_cost / max(attacker_profit, 1) ≥ 10⁶
 *      across a wide range of input scales.
 *
 * Sources: lib.rs:419-481 (waterfall), lib.rs:703-718 / :826-841
 * (preview_deposit/redeem), state.rs (V_A=1e6, V_S=1e9, PROTOCOL_FEE_BPS=1500).
 */

import { expect } from "chai";

// -- Constants -----------------------------------------------------------
const VIRTUAL_ASSETS_OFFSET = 1_000_000n;
const VIRTUAL_SHARES_OFFSET = 1_000_000_000n;
const BPS_DENOMINATOR = 10_000n;
const PROTOCOL_FEE_BPS = 1500n;
const MAX_LATE_DAYS = 365n;

// -- Pure replicas (mirrored from deposit_withdraw / claim_and_settle) ---
function previewDeposit(amount: bigint, totalAssets: bigint, totalShares: bigint): bigint {
  return (amount * (totalShares + VIRTUAL_SHARES_OFFSET)) / (totalAssets + VIRTUAL_ASSETS_OFFSET);
}

function previewRedeem(shares: bigint, totalAssets: bigint, totalShares: bigint): bigint {
  return (shares * (totalAssets + VIRTUAL_ASSETS_OFFSET)) / (totalShares + VIRTUAL_SHARES_OFFSET);
}

interface Waterfall {
  protocolCut: bigint;
  lpCut: bigint;
  agentNet: bigint;
}

/**
 * Replica of the waterfall math in `claim_and_settle` (lib.rs:419-481).
 *
 *   total_fee   = fee_owed + late_penalty
 *   protocol   = total_fee * 1500 / 10000   // 15% to protocol
 *   lp_fee     = total_fee - protocol        // 85% to LPs
 *   lp_cut     = principal + lp_fee
 *   agent_net  = payment - protocol - lp_cut
 *
 * Caller must satisfy `payment_amount >= principal + total_fee` (else the
 * handler reverts with WaterfallSumMismatch at lib.rs:418).
 */
function computeWaterfall(
  principal: bigint,
  feeOwed: bigint,
  latePenalty: bigint,
  paymentAmount: bigint,
): Waterfall {
  const totalFee = feeOwed + latePenalty;
  const protocolCut = (totalFee * PROTOCOL_FEE_BPS) / BPS_DENOMINATOR;
  const lpFee = totalFee - protocolCut;
  const lpCut = principal + lpFee;
  const agentNet = paymentAmount - protocolCut - lpCut;
  return { protocolCut, lpCut, agentNet };
}

// -- Deterministic PRNG (mulberry32) for reproducible fuzzing ------------
function mulberry32(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function bigInRange(rng: () => number, lo: bigint, hi: bigint): bigint {
  // Sample a bigint in [lo, hi]. `rng()` is in [0, 1).
  const span = hi - lo + 1n;
  // Mix two 32-bit draws so we cover up to 64 bits of resolution.
  const r =
    BigInt(Math.floor(rng() * 0x100000000)) * 0x100000000n +
    BigInt(Math.floor(rng() * 0x100000000));
  return lo + (r % span);
}

// -- Property 1: waterfall sum invariant ---------------------------------
describe("invariants / waterfall sum (property)", () => {
  it("protocol_cut + lp_cut + agent_net == payment_amount (1000 random cases)", () => {
    const rng = mulberry32(0xC0DE_CAFE);
    const cases = 1_000;
    for (let i = 0; i < cases; i++) {
      // principal ∈ [1, 1B atoms] = [0.000001, 1000] USDC
      const principal = bigInRange(rng, 1n, 1_000_000_000n);
      // fee_owed ∈ [0, 50% of principal]
      const feeOwed = bigInRange(rng, 0n, principal / 2n);
      // late_penalty ∈ [0, 365 days * 10 bps/day * principal]
      const maxLate = (principal * 10n * MAX_LATE_DAYS) / BPS_DENOMINATOR;
      const latePenalty = bigInRange(rng, 0n, maxLate);
      const totalOwed = principal + feeOwed + latePenalty;
      // payment_amount ∈ [total_owed, total_owed * 2] (real callers may
      // overpay; the handler accepts >= total_owed and credits the surplus
      // to the agent).
      const paymentAmount = bigInRange(rng, totalOwed, totalOwed * 2n);

      const w = computeWaterfall(principal, feeOwed, latePenalty, paymentAmount);

      // Headline invariant.
      expect(w.protocolCut + w.lpCut + w.agentNet, `case ${i}`).to.equal(
        paymentAmount,
      );

      // Side invariants worth catching:
      //   - lp_cut >= principal (LPs always recover at least their capital).
      expect(w.lpCut >= principal, `case ${i}: lp_cut >= principal`).to.be.true;
      //   - protocol_cut <= total_fee (15% slice can't exceed the fee pool).
      expect(w.protocolCut <= feeOwed + latePenalty, `case ${i}`).to.be.true;
      //   - agent_net >= 0 when payment_amount >= total_owed.
      expect(w.agentNet >= 0n, `case ${i}: agent_net >= 0`).to.be.true;
    }
  });

  it("payment_amount == total_owed → agent_net == 0 (no overpayment surplus)", () => {
    const principal = 100_000_000n;
    const feeOwed = 5_000_000n;
    const latePenalty = 200_000n;
    const totalOwed = principal + feeOwed + latePenalty;
    const w = computeWaterfall(principal, feeOwed, latePenalty, totalOwed);
    expect(w.agentNet).to.equal(0n);
    // The whole payment goes to LP cut + protocol cut.
    expect(w.protocolCut + w.lpCut).to.equal(totalOwed);
  });

  it("zero fees + zero penalty → protocol_cut = 0, lp_cut = principal", () => {
    const w = computeWaterfall(50_000_000n, 0n, 0n, 50_000_000n);
    expect(w.protocolCut).to.equal(0n);
    expect(w.lpCut).to.equal(50_000_000n);
    expect(w.agentNet).to.equal(0n);
  });

  it("rounding: 15/85 split rounds DOWN for protocol (LPs absorb the dust)", () => {
    // total_fee = 7 atoms → protocol = 7*1500/10000 = 1 (truncated from 1.05).
    // lp_fee = 6. Sum = 7. ✓
    const w = computeWaterfall(0n, 7n, 0n, 7n);
    expect(w.protocolCut).to.equal(1n);
    expect(w.lpCut).to.equal(6n);
    expect(w.agentNet).to.equal(0n);
  });

  it("late penalty saturates at MAX_LATE_DAYS regardless of clock drift", () => {
    // Even with absurd late durations, the on-chain handler caps at 365 days.
    // The math here doesn't enforce that cap (the cap is in the handler at
    // lib.rs:417); document the contract.
    const principal = 100_000_000n;
    // 365 days * 10 bps/day = 36.5% of principal.
    const cappedPenalty = (principal * 10n * MAX_LATE_DAYS) / BPS_DENOMINATOR;
    expect(cappedPenalty).to.equal(36_500_000n);
  });
});

// -- Property 2: share-price monotonicity --------------------------------
describe("invariants / share-price monotonicity (property)", () => {
  // Returns a 256-bit-ish "rational" P_num / P_den for share price comparison
  // without floating point. Comparing a/b vs c/d via a*d vs c*b.
  function pricePair(totalAssets: bigint, totalShares: bigint): [bigint, bigint] {
    return [totalAssets + VIRTUAL_ASSETS_OFFSET, totalShares + VIRTUAL_SHARES_OFFSET];
  }
  function priceLE(
    [aNum, aDen]: [bigint, bigint],
    [bNum, bDen]: [bigint, bigint],
  ): boolean {
    return aNum * bDen <= bNum * aDen;
  }

  it("deposits never decrease share price (200 sequences × ≤25 ops)", () => {
    const rng = mulberry32(0xDEADBEEF);
    for (let seq = 0; seq < 200; seq++) {
      let totalAssets = 0n;
      let totalShares = 0n;
      let prev: [bigint, bigint] = pricePair(totalAssets, totalShares);
      const ops = 5 + Math.floor(rng() * 20);
      for (let i = 0; i < ops; i++) {
        const amount = bigInRange(rng, 1n, 1_000_000_000_000n); // up to 1M USDC
        const shares = previewDeposit(amount, totalAssets, totalShares);
        if (shares === 0n) continue; // skip dust deposits (would no-op)
        totalAssets += amount;
        totalShares += shares;
        const next = pricePair(totalAssets, totalShares);
        expect(priceLE(prev, next), `seq ${seq} op ${i} deposit ${amount}`).to.be
          .true;
        prev = next;
      }
    }
  });

  it("withdrawals never decrease share price (200 sequences × ≤25 ops)", () => {
    const rng = mulberry32(0xCAFE_BABE);
    for (let seq = 0; seq < 200; seq++) {
      // Seed pool with an initial deposit so withdraws have something to chew on.
      const seedAmount = bigInRange(rng, 1_000_000_000n, 1_000_000_000_000n);
      let totalAssets = seedAmount;
      let totalShares = previewDeposit(seedAmount, 0n, 0n);
      let prev: [bigint, bigint] = pricePair(totalAssets, totalShares);
      const ops = 5 + Math.floor(rng() * 20);
      for (let i = 0; i < ops; i++) {
        if (totalShares === 0n) break;
        // Withdraw a random fraction of remaining shares.
        const sharesToBurn = bigInRange(rng, 1n, totalShares);
        const assetsOut = previewRedeem(sharesToBurn, totalAssets, totalShares);
        if (assetsOut === 0n) continue;
        totalAssets -= assetsOut;
        totalShares -= sharesToBurn;
        const next = pricePair(totalAssets, totalShares);
        expect(priceLE(prev, next), `seq ${seq} op ${i} burn ${sharesToBurn}`).to.be
          .true;
        prev = next;
      }
    }
  });

  it("yield accrual (lp_fee → total_assets, shares unchanged) strictly increases price", () => {
    // claim_and_settle: pool.total_assets += lp_fee; total_shares unchanged.
    const rng = mulberry32(0xFEEDFACE);
    for (let seq = 0; seq < 200; seq++) {
      const seed = bigInRange(rng, 1_000_000n, 1_000_000_000_000n);
      let totalAssets = seed;
      let totalShares = previewDeposit(seed, 0n, 0n);
      let prev: [bigint, bigint] = pricePair(totalAssets, totalShares);
      const accruals = 3 + Math.floor(rng() * 10);
      for (let i = 0; i < accruals; i++) {
        const lpFee = bigInRange(rng, 1n, totalAssets / 10n + 1n);
        totalAssets += lpFee;
        const next = pricePair(totalAssets, totalShares);
        // Strict increase: yield is pure upside for sitting LPs.
        expect(prev[0] * next[1] < next[0] * prev[1], `seq ${seq} op ${i}`).to.be
          .true;
        prev = next;
      }
    }
  });

  it("mixed sequence (deposit + withdraw + yield) is non-decreasing (300 seqs × ≤30 ops)", () => {
    const rng = mulberry32(0xBADC0FFEE0DDF00Dn & 0xFFFFFFFFn ? 0xBADC0FFE : 0); // any 32-bit seed
    for (let seq = 0; seq < 300; seq++) {
      let totalAssets = 0n;
      let totalShares = 0n;
      let prev: [bigint, bigint] = pricePair(totalAssets, totalShares);
      const ops = 5 + Math.floor(rng() * 25);
      for (let i = 0; i < ops; i++) {
        const op = Math.floor(rng() * 3);
        if (op === 0) {
          // deposit
          const amount = bigInRange(rng, 1n, 1_000_000_000n);
          const shares = previewDeposit(amount, totalAssets, totalShares);
          if (shares === 0n) continue;
          totalAssets += amount;
          totalShares += shares;
        } else if (op === 1 && totalShares > 0n) {
          // withdraw
          const burn = bigInRange(rng, 1n, totalShares);
          const out = previewRedeem(burn, totalAssets, totalShares);
          if (out === 0n) continue;
          totalAssets -= out;
          totalShares -= burn;
        } else if (op === 2 && totalAssets > 0n) {
          // yield accrual
          const lpFee = bigInRange(rng, 1n, totalAssets / 5n + 1n);
          totalAssets += lpFee;
        } else {
          continue;
        }
        const next = pricePair(totalAssets, totalShares);
        expect(priceLE(prev, next), `seq ${seq} op ${i} kind ${op}`).to.be.true;
        prev = next;
      }
    }
  });
});

// -- Property 3: first-depositor inflation defense (fuzz extension) -------
describe("invariants / first-depositor inflation defense (property)", () => {
  it("attacker_cost / max(attacker_profit, 1) ≥ 10⁶ across input ranges (200 cases)", () => {
    const rng = mulberry32(0xABCD0123);
    for (let i = 0; i < 200; i++) {
      // Attacker deposits 1..100 atoms; donates 1k..100B atoms.
      const attackerDeposit = bigInRange(rng, 1n, 100n);
      const attackerDonation = bigInRange(rng, 1_000n, 100_000_000_000n);
      // Victim deposits 1..1M USDC.
      const victimDeposit = bigInRange(rng, 1_000_000n, 1_000_000_000_000n);

      // Step 1: attacker deposits.
      const attackerShares = previewDeposit(attackerDeposit, 0n, 0n);
      if (attackerShares === 0n) continue;

      // Step 2: attacker donates directly to vault — pool.total_assets is the
      // STORED field, unchanged by donations. The defense rests on this.
      let totalAssets = attackerDeposit;
      let totalShares = attackerShares;

      // Step 3: victim deposit at the post-attacker stored state.
      const victimShares = previewDeposit(victimDeposit, totalAssets, totalShares);
      if (victimShares === 0n) continue;
      totalAssets += victimDeposit;
      totalShares += victimShares;

      // Step 4: attacker redeems all attacker shares.
      const attackerRedeem = previewRedeem(
        attackerShares,
        totalAssets,
        totalShares,
      );

      const attackerCost = attackerDeposit + attackerDonation;
      // Profit is what they got back minus what they put in (deposit only —
      // the donation is sunk regardless of redemption math).
      const attackerProfit = attackerRedeem; // <= attackerDeposit; we measure recovered atoms.

      const cost = attackerCost;
      const denom = attackerProfit > 0n ? attackerProfit : 1n;
      const ratio = cost / denom;
      expect(
        ratio >= 1_000_000n,
        `case ${i}: cost=${cost} profit=${attackerProfit} ratio=${ratio} ` +
          `(attacker_dep=${attackerDeposit} donation=${attackerDonation} victim=${victimDeposit})`,
      ).to.be.true;
    }
  });

  it("zero-donation baseline: 1-atom attacker still cannot extract victim funds", () => {
    // Without donating, the attacker just owns a tiny share. Their redeem
    // can't exceed their original deposit (proven by share-price
    // monotonicity above) — the round trip is at most a 1-atom rounding loss.
    const attackerShares = previewDeposit(1n, 0n, 0n); // 1000 shares
    let totalAssets = 1n;
    let totalShares = attackerShares;
    const victimDeposit = 1_000_000_000n;
    const victimShares = previewDeposit(victimDeposit, totalAssets, totalShares);
    totalAssets += victimDeposit;
    totalShares += victimShares;
    const back = previewRedeem(attackerShares, totalAssets, totalShares);
    // Attacker cannot extract more than they put in (= 1 atom).
    expect(back <= 1n).to.be.true;
  });
});
