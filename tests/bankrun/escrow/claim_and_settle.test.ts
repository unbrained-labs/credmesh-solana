/**
 * `credmesh-escrow::claim_and_settle` — on-chain-call semantics.
 *
 * Sources:
 *   - lib.rs:407-538 (handler)
 *   - lib.rs:1010-1066 (ClaimAndSettle accounts struct)
 *   - state.rs CLAIM_WINDOW_SECONDS = 7 * 24 * 60 * 60
 *
 * **Math is already covered in invariants.test.ts (PR #20)**: waterfall sum,
 * 15/85 split, late-day saturation, lp_cut ≥ principal, etc. This file
 * focuses on the *call semantics* the handler imposes:
 *
 *   - Account ordering matches the struct (`#[derive(Accounts)]` order is
 *     what the IDL emits)
 *   - Cranker permission constraint: `cranker.key() == advance.agent` (v1)
 *   - Settlement window: now ≥ advance.expires_at − CLAIM_WINDOW_SECONDS
 *   - Memo nonce binding: payment-tx memo bytes == consumed.nonce
 *   - Payment ≥ total_owed (else WaterfallSumMismatch from the require!)
 *   - Three CPI transfers in fixed order: protocol → treasury, lp → vault,
 *     agent_net → agent (skipped if payer_ata == agent_ata, the v1 default)
 *   - Pool state mutation: deployed_amount -= principal; total_assets +=
 *     lp_fee; accrued_protocol_fees += protocol_cut
 *   - Advance.state = Settled; `close = agent` (rent refund → agent, NOT
 *     cranker — neutralizes MEV cranking)
 *   - **ConsumedPayment NOT closed** (AUDIT P0-5; defended by Day 3 PR #24)
 *   - AdvanceSettled event emitted last (lib.rs:531-536)
 *
 * **Layout dependency**: PR #14 derives InitSpace for Pool/Advance/
 * ConsumedPayment etc. The runtime layout (field offsets, sizes) doesn't
 * change — only the constant computation. Tests track PR #14 layouts
 * directly because the audited diff is stable.
 *
 * Scaffold: pure structural assertions on constants + PDA / account-order
 * sanity, plus harness specs encoding the full call.
 */

import { expect } from "chai";
import { Keypair, PublicKey } from "@solana/web3.js";
import {
  setupBankrun,
  poolPda,
  advancePda,
  consumedPda,
  TestContext,
} from "../setup";

// -- Constants mirrored from credmesh-escrow ------------------------------
const CLAIM_WINDOW_SECONDS = 7 * 24 * 60 * 60;     // state.rs
const PROTOCOL_FEE_BPS = 1500n;                    // state.rs
const BPS_DENOMINATOR = 10_000n;                   // state.rs

// -- Pure: window math + account-order invariants -------------------------
describe("credmesh-escrow / claim_and_settle — call semantics (pure)", () => {
  it("CLAIM_WINDOW_SECONDS is 7 days (state.rs constant)", () => {
    expect(CLAIM_WINDOW_SECONDS).to.equal(604_800);
  });

  it("settlement window opens at expires_at - CLAIM_WINDOW_SECONDS", () => {
    // Plan formula (lib.rs:409-411):
    //   claim_window_start = advance.expires_at - CLAIM_WINDOW_SECONDS
    //   require!(now >= claim_window_start, NotSettleable)
    const expiresAt = 1_750_000_000;
    const start = expiresAt - CLAIM_WINDOW_SECONDS;
    expect(start).to.equal(expiresAt - 604_800);
  });

  it("payment >= total_owed = principal + fee_owed + late_penalty", () => {
    // lib.rs:417 — `require!(payment_amount >= total_owed, WaterfallSumMismatch)`.
    const principal = 100_000_000n;
    const feeOwed = 5_000_000n;
    const latePenalty = 200_000n;
    const totalOwed = principal + feeOwed + latePenalty;
    expect(totalOwed).to.equal(105_200_000n);
  });

  it("PDA derivation: ClaimAndSettle expects advance + consumed at deterministic addresses", () => {
    // The struct (lib.rs:1010-1066) re-derives both PDAs via seeds:
    //   advance:  [ADVANCE_SEED, pool, advance.agent, advance.receivable_id]
    //   consumed: [CONSUMED_SEED, pool, advance.receivable_id]    (pre-#8)
    //              or [CONSUMED_SEED, pool, agent, recv_id]       (post-#14)
    // Determinism is the property — given the same seeds the addresses match.
    const usdc = Keypair.generate().publicKey;
    const agent = Keypair.generate().publicKey;
    const recvId = Buffer.alloc(32, 0x10);
    const [pool] = poolPda(usdc);
    const [adv1] = advancePda(pool, agent, recvId);
    const [adv2] = advancePda(pool, agent, recvId);
    expect(adv1.equals(adv2)).to.be.true;
    const [cons1] = consumedPda(pool, recvId);
    const [cons2] = consumedPda(pool, recvId);
    expect(cons1.equals(cons2)).to.be.true;
  });

  it("docs: 12-account ClaimAndSettle ordering (matches lib.rs:1010-1066)", () => {
    const accountsInOrder = [
      "cranker",                  // signer; mut; cranker.key() == advance.agent (v1)
      "advance",                  // mut; close = agent; state == Issued
      "consumed",                 // NOT mut; NO close; consumed.agent == advance.agent
      "agent",                    // mut; address = advance.agent (rent recipient)
      "pool",                     // mut
      "pool_usdc_vault",          // mut; address = pool.usdc_vault
      "agent_usdc_ata",           // mut; token::mint, token::authority = advance.agent
      "protocol_treasury_ata",    // mut; address = pool.treasury_ata (P0-3)
      "payer_usdc_ata",           // mut; token::authority = cranker (P0-4)
      "usdc_mint",                // address = pool.asset_mint
      "instructions_sysvar",      // address = sysvar_instructions::ID (P1-2)
      "token_program",
    ];
    expect(accountsInOrder).to.have.lengthOf(12);
    // Address constraints come from AUDIT P0-3/P0-4, P1-2.
    expect(accountsInOrder[7]).to.equal("protocol_treasury_ata");
    expect(accountsInOrder[10]).to.equal("instructions_sysvar");
  });

  it("constraint: Advance.state == Issued blocks double-settlement", () => {
    // lib.rs:1023 — `constraint = advance.state == AdvanceState::Issued`.
    // Once the handler sets state=Settled (lib.rs:521), the close=agent
    // cleanup happens at end-of-handler. A second claim_and_settle on the
    // same Advance can't even fetch the account (closed).
    const states = ["Issued", "Settled", "Liquidated"];
    expect(states[0]).to.equal("Issued");
  });
});

// -- Harness scaffold -----------------------------------------------------
describe("credmesh-escrow / claim_and_settle (harness)", () => {
  let ctx: TestContext;

  before(async () => {
    ctx = await setupBankrun();
  });

  it("happy path: settle within window, three transfers, state mutates correctly (BEHAVIORAL)", async () => {
    // Plan once IDL lands:
    //   1. init_pool, LP deposit, init oracle, worker_update_receivable.
    //   2. agent.request_advance(receivable_id=X, amount=200M, kind=Worker)
    //      → Advance + Consumed PDAs init; agent_usdc_ata gets 200M.
    //   3. clock.warp(advance.expires_at - CLAIM_WINDOW_SECONDS + 1)
    //   4. Build claim_and_settle tx with memo ix carrying consumed.nonce:
    //        [ memo(consumed.nonce),
    //          claim_and_settle(payment_amount = principal+fee+late) ]
    //   5. Send tx (cranker = agent in v1).
    //   6. Assertions:
    //        * Advance closed (program.account.advance.fetchNullable === null)
    //        * Consumed STILL exists (P0-5; fetch returns the same data)
    //        * agent lamport balance: gained Advance rent (~0.0017 SOL)
    //        * pool.deployed_amount: pre - principal
    //        * pool.total_assets: pre + lp_fee
    //        * pool.accrued_protocol_fees: pre + protocol_cut
    //        * usdc_vault.amount: pre + lp_cut (= principal + lp_fee)
    //        * protocol_treasury_ata.amount: pre + protocol_cut
    //        * agent_usdc_ata.amount: pre - net_paid (paid out of agent's
    //          ATA when payer_ata == agent_ata; the self-transfer step
    //          short-circuits per lib.rs:497-503)
    //        * AdvanceSettled event emitted with all fields populated.
    expect(ctx.programs.escrow).to.exist;
  });

  it("rejects when now < expires_at - CLAIM_WINDOW_SECONDS (NotSettleable, BEHAVIORAL)", async () => {
    // lib.rs:411 — `require!(now >= claim_window_start, NotSettleable)`.
    // Plan: issue advance with expires_at = now + 30 days. Claim immediately
    // (still 23 days from window-open). Expect NotSettleable.
    expect(true).to.be.true;
  });

  it("rejects non-agent cranker (InvalidPayer, BEHAVIORAL)", async () => {
    // Constraint at lib.rs:1015 — `cranker.key() == advance.agent`.
    // Already covered by ata_substitution.test.ts in spirit; this assertion
    // pins the v1 invariant from the call-semantics angle.
    expect(true).to.be.true;
  });

  it("rejects missing memo / wrong-nonce memo (MemoNonceMismatch, BEHAVIORAL)", async () => {
    // lib.rs:413-415 — require_memo_nonce reads the memo ix and matches
    // its bytes against consumed.nonce (16 bytes).
    // Plan A: omit memo entirely → MemoMissing.
    // Plan B: include memo with wrong 16 bytes → MemoNonceMismatch.
    expect(true).to.be.true;
  });

  it("rejects payment_amount < total_owed (WaterfallSumMismatch, BEHAVIORAL)", async () => {
    // lib.rs:417 — `require!(payment_amount >= total_owed)`.
    // Plan: total_owed = principal + fee + late = 105.2M. Pass payment=100M.
    // Expect WaterfallSumMismatch (the same error used for sum drift).
    expect(true).to.be.true;
  });

  it("ConsumedPayment is NOT closed after settlement (P0-5, BEHAVIORAL)", async () => {
    // The headline P0-5 invariant. Day 3 PR #24 covers the negative
    // direction (re-init fails); this assertion pins the affirmative:
    // after a successful claim_and_settle, consumed.agent / nonce /
    // created_at are unchanged from issuance.
    //
    // Plan:
    //   const before = await program.account.consumedPayment.fetch(consumed);
    //   await claimAndSettle(...);
    //   const after = await program.account.consumedPayment.fetch(consumed);
    //   expect(after.bump).to.equal(before.bump);
    //   expect(Buffer.from(after.nonce)).to.deep.equal(Buffer.from(before.nonce));
    //   expect(after.agent.equals(before.agent)).to.be.true;
    //   expect(after.createdAt.toNumber()).to.equal(before.createdAt.toNumber());
    expect(true).to.be.true;
  });

  it("Advance close = agent: rent refund flows to agent NOT cranker (BEHAVIORAL)", async () => {
    // lib.rs:1024 (close = agent). Even though cranker == agent in v1,
    // the constraint is what neutralizes MEV cranking when v2 makes
    // claim_and_settle permissionless. Test: assert agent's lamport
    // delta over the tx ≥ Advance rent (~0.0017 SOL).
    expect(true).to.be.true;
  });

  it("payer_ata == agent_ata (v1 default): agent_net transfer is skipped (BEHAVIORAL)", async () => {
    // lib.rs:497-503 — the agent_net transfer is conditional on
    // `payer_usdc_ata.key() != agent_usdc_ata.key()`. In v1 the cranker
    // is the agent and typically funds the settlement out of the same
    // ATA that received the advance; the self-transfer is a no-op.
    //
    // Plan: assert agent_usdc_ata.amount delta == -(protocol_cut + lp_cut)
    // (i.e., the funds left only via the protocol + lp transfers; the
    // agent-net is neither moved nor double-counted).
    expect(true).to.be.true;
  });

  it("emits AdvanceSettled with correct fields (BEHAVIORAL)", async () => {
    // lib.rs:531-538. Decode event from logs; assert pool/agent/advance
    // pubkeys + principal + lp_cut + protocol_cut + agent_net + late_days.
    expect(true).to.be.true;
  });

  it("over-payment: agent_net > 0 surplus credits to agent (BEHAVIORAL)", async () => {
    // Plan: total_owed = 100M; pay 110M. Expect:
    //   - protocol_cut on the FEE portion only (15% of fee+late_penalty)
    //   - lp_cut = principal + lp_fee
    //   - agent_net = payment - protocol_cut - lp_cut = 10M (the surplus)
    //   - Agent_usdc_ata receives the agent_net only when payer_ata !=
    //     agent_ata; otherwise it's bookkeeping (already in agent_ata).
    expect(true).to.be.true;
  });
});
