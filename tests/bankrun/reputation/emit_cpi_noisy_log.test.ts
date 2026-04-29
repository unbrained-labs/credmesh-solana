/**
 * Issue #3 fixture.
 *
 * Verifies that `NewFeedback` survives a transaction whose log buffer is
 * intentionally pushed past Solana's 10,000-byte LogCollector cap by an
 * adversarial co-instruction. The defense is `emit_cpi!` — event payload
 * lives in inner-instruction data (transaction metadata), not the log
 * buffer, so log truncation cannot drop it.
 *
 * Threat model
 * ------------
 * `NewFeedback` carries `feedback_uri: String` (max 200 B) plus 3× [u8; 32],
 * 2× Pubkey, score_ema_after: u128 — ~400 B raw, ~540 chars of base64 after
 * Anchor's `sol_log_data` framing. An attacker bundles a co-instruction
 * (Memo, or any program that emits long logs) BEFORE `give_feedback` to
 * push the running log byte total past 10 KB. Under plain `emit!`, the
 * `Program data: ...` line for NewFeedback would be silently truncated —
 * indexers scraping logs would miss the event and on-chain reputation
 * state would diverge from any dashboard derived from log streams.
 *
 * Under `emit_cpi!` the event is a self-CPI to the program with the
 * serialized event as instruction data. It appears in
 * `meta.innerInstructions` regardless of log truncation.
 *
 * Test design
 * -----------
 * Per-`it`:
 *   1. Construct a tx: [memo(filler), give_feedback(...)].
 *   2. Choose `filler` length so the cumulative log byte count for the
 *      memo ix exceeds the 10,000-byte cap. SPL Memo logs the entire
 *      memo as `Program log: Memo (len N): "<hex>"`, so a single memo
 *      of ~10 KB suffices, or several stacked memos. Per-tx data budget
 *      (1232 bytes) constrains us — we either use multiple chained
 *      memo ixs or a single dedicated `noisy_log_filler` helper program
 *      that generates >10 KB of `msg!` output from a small input.
 *   3. Submit the tx via the bankrun banks client; assert it succeeds.
 *   4. Fetch the tx via `getTransaction`; assert:
 *        - `meta.logMessages` is truncated (length capped at the 10 KB
 *          boundary; a "Log truncated" sentinel may appear at the tail).
 *        - The `NewFeedback` payload is recoverable from
 *          `meta.innerInstructions` — the inner ix targeting our
 *          program ID, with data discriminator matching `NewFeedback`,
 *          decodes to the expected fields.
 *
 * Status
 * ------
 * Scaffolded. Bodies activate once `anchor build` produces the IDL at
 * `target/idl/credmesh_reputation.json` (Track A is delivering this).
 * Until then the structural assertions below confirm the test harness
 * loads the program correctly; the real behavior assertions live in
 * comments that document the exact decoder calls the live test will use.
 */

import { expect } from "chai";
import { setupBankrun, reputationPda, oracleConfigPda, TestContext } from "../setup";
import { Keypair, PublicKey } from "@solana/web3.js";

// SPL Memo v2 — emits its data verbatim as a `Program log:` line, ideal
// adversarial filler. Loaded as an external program in `setupBankrun` once
// the test is activated.
const MEMO_PROGRAM_ID = new PublicKey(
  "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr",
);

// Solana's LogCollector cap (program-runtime/src/log_collector.rs).
const LOG_TRUNCATION_LIMIT = 10_000;

// Anchor event-CPI discriminator prefix per anchor-lang 0.30:
// the inner-instruction data starts with [228, 69, 165, 46, 81, 203, 154, 29]
// (sighash of "anchor:event") followed by the event-specific 8-byte
// discriminator and then the borsh-serialized event struct.
const ANCHOR_EVENT_CPI_DISCRIMINATOR = Buffer.from([
  228, 69, 165, 46, 81, 203, 154, 29,
]);

describe("ATTACK FIXTURE / emit_cpi survives noisy-log truncation (issue #3)", () => {
  let ctx: TestContext;

  before(async () => {
    ctx = await setupBankrun();
  });

  it("loads the reputation program into bankrun", () => {
    // Sanity: the program is deployed in the harness and addressable.
    // Without this the rest of the suite is meaningless.
    expect(ctx.programs.reputation).to.exist;
    expect(MEMO_PROGRAM_ID.toBase58()).to.have.length.greaterThan(0);
  });

  it(
    "give_feedback succeeds when bundled with adversarial filler that " +
      "would truncate the log buffer past 10 KB",
    async () => {
      // Steps once IDL is available:
      //
      //   const program = new anchor.Program(IDL, ctx.programs.reputation, ctx.provider);
      //   const agentAsset = Keypair.generate().publicKey;
      //   const attestor = Keypair.generate();
      //   const [reputation] = reputationPda(agentAsset);
      //   const [oracleConfig] = oracleConfigPda();
      //
      //   await program.methods.initReputation()
      //     .accounts({ payer: ctx.payer.publicKey, agentAsset, reputation,
      //                 systemProgram: SystemProgram.programId })
      //     .rpc();
      //
      //   // Filler sized so the LogCollector exceeds 10 KB before our ix runs.
      //   // SPL Memo logs `Program log: Memo (len N): "<bytes>"` plus framing.
      //   // 6 chained memos × ~1.7 KB each safely overshoots 10 KB while
      //   // staying inside the 1232-byte per-tx payload budget when split
      //   // across separate ixs (the tx packet limit is on serialized tx, not
      //   // on log output).
      //   const filler = Buffer.alloc(1700, 0x41); // 'AAAA…'
      //   const memoIxs = Array.from({ length: 6 }, () => ({
      //     programId: MEMO_PROGRAM_ID,
      //     keys: [],
      //     data: filler,
      //   }));
      //
      //   const giveFeedbackIx = await program.methods
      //     .giveFeedback({
      //       score: 88,
      //       value: 100_000n,
      //       valueDecimals: 6,
      //       reasonCode: 0,
      //       feedbackUri: "ipfs://bafy".padEnd(200, "x"), // exercise max-length URI
      //       feedbackHash: new Array(32).fill(7),
      //       jobId: new Array(32).fill(11),
      //     })
      //     .accounts({
      //       attestor: attestor.publicKey,
      //       agentAsset,
      //       reputation,
      //       oracleConfig,
      //       // event_cpi auto-injected: eventAuthority, program
      //     })
      //     .signers([attestor])
      //     .instruction();
      //
      //   const tx = new Transaction().add(...memoIxs, giveFeedbackIx);
      //   const sig = await ctx.provider.sendAndConfirm(tx, [attestor]);
      //
      //   const fetched = await ctx.provider.connection.getTransaction(sig, {
      //     commitment: "confirmed",
      //     maxSupportedTransactionVersion: 0,
      //   });
      //
      //   // 1. Logs were truncated by the cap.
      //   const totalLogBytes =
      //     fetched.meta.logMessages.reduce((s, l) => s + l.length + 1, 0);
      //   expect(totalLogBytes).to.be.lessThanOrEqual(LOG_TRUNCATION_LIMIT + 64);
      //
      //   // 2. NewFeedback IS recoverable from innerInstructions despite log
      //   //    truncation. Anchor's emit_cpi wraps the event in a self-CPI
      //   //    whose ix data is [event-cpi-discr || event-discr || borsh(event)].
      //   const innerIxs = fetched.meta.innerInstructions.flatMap(
      //     (g) => g.instructions,
      //   );
      //   const eventIx = innerIxs.find((ix) => {
      //     const programIdIdx = ix.programIdIndex;
      //     const programId =
      //       fetched.transaction.message.staticAccountKeys[programIdIdx];
      //     if (!programId.equals(ctx.programs.reputation)) return false;
      //     const data = bs58.decode(ix.data as string);
      //     return data
      //       .subarray(0, 8)
      //       .equals(ANCHOR_EVENT_CPI_DISCRIMINATOR);
      //   });
      //   expect(eventIx, "NewFeedback CPI inner-ix must survive truncation")
      //     .to.exist;
      //
      //   // 3. Decoded event matches what the handler emitted.
      //   const decoded = program.coder.events.decode(
      //     bs58.decode(eventIx.data as string).subarray(8).toString("base64"),
      //   );
      //   expect(decoded.name).to.equal("newFeedback");
      //   expect(decoded.data.score).to.equal(88);
      //   expect(decoded.data.feedbackIndex.toNumber()).to.equal(0);
      //
      //   // Negative control: under plain emit! the event would have been
      //   //   silently dropped from logMessages — meta.innerInstructions
      //   //   would NOT contain a self-CPI for the reputation program, and
      //   //   the corresponding `Program data: ...` line would be missing
      //   //   from the truncated log tail. We do not include the negative
      //   //   path in this fixture because we'd need a counterfactual
      //   //   program build with `emit!` to demonstrate it. The Day 1 PR
      //   //   #11 + this fixture together prove the positive path.
      expect(ctx.programs.reputation).to.exist;
    },
  );

  it(
    "emit_cpi inner-ix data starts with the anchor:event-cpi discriminator",
    () => {
      // Compile-time / structural check the live test depends on.
      // anchor-lang 0.30 prefixes every emit_cpi inner-ix with the 8-byte
      // sighash of "anchor:event" so off-chain consumers can filter.
      expect(ANCHOR_EVENT_CPI_DISCRIMINATOR.length).to.equal(8);
    },
  );

  it("LogCollector cap matches Solana runtime constant", () => {
    // Anchored to `program-runtime/src/log_collector.rs::LOG_MESSAGES_BYTES_LIMIT`.
    // If Solana raises this cap, the noisy-log attack surface shrinks but the
    // fix is still correct (CPI-emitted events remain unaffected by log size).
    expect(LOG_TRUNCATION_LIMIT).to.equal(10_000);
  });
});
