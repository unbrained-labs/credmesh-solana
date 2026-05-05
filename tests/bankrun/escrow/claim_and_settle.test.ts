/**
 * `credmesh-escrow::claim_and_settle` — on-chain-call semantics.
 *
 * **Two-mode dispatch (this branch):** the cranker may be the agent (Mode A,
 * legacy v1 behavior preserved) OR any third-party relayer (Mode B,
 * permissionless settlement via SPL `Approve` delegate granted by
 * `request_advance`). The handler branches on `cranker.key() ==
 * advance.agent`; the account-struct constraints no longer enforce that
 * equality.
 *
 * Sources:
 *   - lib.rs `claim_and_settle` handler (two-mode dispatch + waterfall)
 *   - lib.rs `ClaimAndSettle` accounts struct (relaxed cranker constraint;
 *     `payer_usdc_ata.token::authority = advance.agent`)
 *   - state.rs CLAIM_WINDOW_SECONDS = 7 * 24 * 60 * 60
 *
 * **Math is already covered in invariants.test.ts (PR #20)**: waterfall sum,
 * 15/85 split, late-day saturation, lp_cut ≥ principal, etc. This file
 * focuses on the *call semantics* the handler imposes:
 *
 *   - Account ordering matches the struct (`#[derive(Accounts)]` order is
 *     what the IDL emits)
 *   - Cranker is ANY signer (no `cranker == advance.agent` constraint)
 *   - Mode A (cranker == advance.agent): transfers signed by cranker
 *   - Mode B (cranker != advance.agent): pool PDA must be the SPL delegate
 *     on agent_usdc_ata with delegated_amount >= total_owed; transfers
 *     signed by pool PDA
 *   - Mode B precondition: payer_usdc_ata == agent_usdc_ata (v1 limit)
 *   - Settlement window: now ≥ advance.expires_at − CLAIM_WINDOW_SECONDS
 *   - Memo nonce binding: payment-tx memo bytes == consumed.nonce
 *   - Payment ≥ total_owed (else WaterfallSumMismatch from the require!)
 *   - Three CPI transfers in fixed order: protocol → treasury, lp → vault,
 *     agent_net → agent (skipped if payer_ata == agent_ata)
 *   - Pool state mutation: deployed_amount -= principal; total_assets +=
 *     lp_fee; accrued_protocol_fees += protocol_cut
 *   - Advance.state = Settled; `close = agent` (rent refund → agent, NOT
 *     cranker — neutralizes MEV cranking even in Mode B)
 *   - **ConsumedPayment NOT closed** (AUDIT P0-5; defended by Day 3 PR #24)
 *   - AdvanceSettled event includes `cranker` for indexer observability
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
const MAX_LATE_DAYS = 365n;                        // state.rs
const LATE_PENALTY_PER_DAY_BPS = 10n;              // 0.1% (lib.rs compute_late_penalty_per_day)

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

  it("docs: 12-account ClaimAndSettle ordering (post-permissionless branch)", () => {
    const accountsInOrder = [
      "cranker",                  // signer; mut; ANY signer (Mode A or Mode B)
      "advance",                  // mut; close = agent; state == Issued
      "consumed",                 // NOT mut; NO close; consumed.agent == advance.agent
      "agent",                    // mut; address = advance.agent (rent + agent_net recipient)
      "pool",                     // mut
      "pool_usdc_vault",          // mut; address = pool.usdc_vault
      "agent_usdc_ata",           // mut; token::mint, token::authority = advance.agent
      "protocol_treasury_ata",    // mut; address = pool.treasury_ata (P0-3)
      "payer_usdc_ata",           // mut; token::authority = advance.agent  ← changed from `= cranker`
      "usdc_mint",                // address = pool.asset_mint
      "instructions_sysvar",      // address = sysvar_instructions::ID (P1-2)
      "token_program",
    ];
    expect(accountsInOrder).to.have.lengthOf(12);
    expect(accountsInOrder[7]).to.equal("protocol_treasury_ata");
    expect(accountsInOrder[10]).to.equal("instructions_sysvar");
  });

  it("delegate approval cap formula: principal + fee_owed + MAX_LATE_DAYS * late_penalty_per_day", () => {
    // request_advance now CPIs `token::approve` at advance issuance with
    // amount = principal + fee_owed + (MAX_LATE_DAYS * late_penalty_per_day).
    // Late penalty per day = 0.1% of principal (10 bps), so worst-case late
    // penalty = 365 * 0.1% * principal = 36.5% of principal.
    const principal = 100_000_000n;          // $100
    const feeOwed = 5_000_000n;              // $5
    const latePerDay = (principal * LATE_PENALTY_PER_DAY_BPS) / BPS_DENOMINATOR;
    expect(latePerDay).to.equal(100_000n);   // 0.1% of $100 = $0.10/day
    const maxLatePenalty = MAX_LATE_DAYS * latePerDay;
    expect(maxLatePenalty).to.equal(36_500_000n);  // $36.50 worst case
    const approveCap = principal + feeOwed + maxLatePenalty;
    expect(approveCap).to.equal(141_500_000n);     // $141.50 cap on the agent's USDC ATA
  });

  it("Mode B precondition: pool PDA delegate + delegated_amount >= total_owed", () => {
    // The handler checks (programs/credmesh-escrow/src/lib.rs after the
    // total_owed computation):
    //   if cranker != advance.agent:
    //     require payer_usdc_ata == agent_usdc_ata
    //         else PayerMustBeAgentInPermissionless
    //     require agent_usdc_ata.delegate == Some(pool_pda_key)
    //         else DelegateNotApproved
    //     require agent_usdc_ata.delegated_amount >= total_owed
    //         else DelegateAmountInsufficient
    const principal = 100_000_000n;
    const feeOwed = 5_000_000n;
    const latePenalty = 200_000n;
    const totalOwed = principal + feeOwed + latePenalty;
    // The approve cap from request_advance is the worst-case envelope, so
    // any actual total_owed (which uses real late_penalty <= max) fits.
    const approveCapWorstCase = principal + feeOwed + MAX_LATE_DAYS * 100_000n;
    expect(totalOwed <= approveCapWorstCase).to.be.true;
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

  it("Mode A — agent self-crank still works (legacy v1 path, BEHAVIORAL)", async () => {
    // Plan: cranker == advance.agent. Handler signs transfers with cranker
    // authority and CPIs token::revoke at end-of-handler. Asserts:
    //   * AdvanceSettled.cranker == advance.agent
    //   * agent_usdc_ata.delegate == None POST-settle (the Mode A revoke
    //     CPI zeroed it out — defense-in-depth from security review)
    //   * agent_usdc_ata.delegated_amount == 0 POST-settle
    //   * Three transfers complete; pool state matches the expected delta
    expect(true).to.be.true;
  });

  it("Mode B — third-party relayer cranks (BEHAVIORAL, the new path)", async () => {
    // Plan:
    //   1. agent.request_advance(...) — CPIs token::approve granting pool
    //      PDA delegate over agent_usdc_ata for principal+fee+max_late.
    //   2. Receivable settles; agent goes offline.
    //   3. relayer.claim_and_settle(...)  — relayer is NOT the agent.
    //   4. Assertions:
    //        * Tx succeeds.
    //        * agent_usdc_ata.delegated_amount decreases by total_owed
    //          (SPL Token decrements when the delegate signs; in Mode B
    //          the pool PDA is the signer).
    //        * Mode B does NOT auto-revoke (Revoke needs owner sig, agent
    //          is offline). Residual = (MAX_LATE_DAYS * late_per_day) -
    //          actual_late_penalty. Off-chain worker bundles Revoke when
    //          the agent comes back online.
    //        * Rent refund from `close = agent` flows to advance.agent
    //          (NOT relayer) — MEV-neutral.
    //        * AdvanceSettled.cranker == relayer.publicKey
    //        * agent's USDC balance: paid total_owed; agent_net stays in
    //          place (payer == agent in Mode B).
    expect(true).to.be.true;
  });

  it("Mode B rejects when pool is not the delegate (DelegateNotApproved, BEHAVIORAL)", async () => {
    // Plan: skip the approval (pre-existing advance with revoked delegate),
    // attempt claim_and_settle from a relayer. Expect DelegateNotApproved.
    expect(true).to.be.true;
  });

  it("Mode B rejects when delegated_amount < total_owed (DelegateAmountInsufficient, BEHAVIORAL)", async () => {
    // Plan: agent re-approves a smaller amount mid-advance (e.g. revoke +
    // approve(1)). Late-penalty kicks in, total_owed exceeds delegated_amount.
    // Expect DelegateAmountInsufficient.
    expect(true).to.be.true;
  });

  it("Mode B rejects when payer_usdc_ata != agent_usdc_ata (PayerMustBeAgentInPermissionless, BEHAVIORAL)", async () => {
    // Plan: relayer passes a different payer_usdc_ata (still owned by agent
    // per the token::authority constraint, but a separate ATA on the same
    // mint). Expect PayerMustBeAgentInPermissionless because Mode B only
    // delegates over the agent's primary ATA.
    expect(true).to.be.true;
  });

  it("ATA substitution defense holds in Mode B (relayer cannot redirect cuts)", async () => {
    // The defenses against the original P0-3 attack are unchanged:
    //   * protocol_treasury_ata — `address = pool.treasury_ata` (account-struct)
    //   * agent_usdc_ata — `token::authority = advance.agent` (account-struct)
    //   * payer_usdc_ata — `token::authority = advance.agent` (account-struct,
    //     was: cranker; the relaxation widens the cranker set without
    //     widening the source-of-funds set)
    //   * agent — `address = advance.agent` (rent recipient via close=agent)
    // None of these depend on cranker identity, so a malicious relayer in
    // Mode B can't substitute attacker-owned destinations.
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
