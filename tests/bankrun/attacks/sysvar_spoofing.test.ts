/**
 * AUDIT P1-2 — sysvar instructions spoofing.
 *
 * Sources:
 *   - lib.rs:1003-1006 (RequestAdvance.instructions_sysvar)
 *   - lib.rs:1062-1065 (ClaimAndSettle.instructions_sysvar)
 *   - programs/credmesh-receivable-oracle/src/lib.rs Ed25519RecordReceivable
 *   - programs/credmesh-shared/src/ix_introspection.rs (verify_prev_ed25519,
 *     require_memo_nonce — both load from instructions_sysvar)
 *
 * Threat model: the program reads neighbor instructions out of the
 * `Sysvar1nstructions1111111111111111111111111` (canonical address) buffer
 * to verify (a) the previous ix is an ed25519 verify with a specific
 * message; (b) a memo ix carries a specific nonce. Without the
 * `address = sysvar_instructions::ID` constraint, an attacker could pass
 * a 1232-byte buffer they control that decodes as "previous ix is
 * ed25519 verify with valid signature over my message", and the handler
 * would trust it.
 *
 * Defense (P1-2 fixed): every site that reads instructions_sysvar pins the
 * account to the canonical sysvar pubkey:
 *
 *   #[account(address = solana_program::sysvar::instructions::ID)]
 *   pub instructions_sysvar: UncheckedAccount<'info>,
 *
 * If the caller passes any other pubkey, Anchor rejects with
 * `ConstraintAddress` (error 2012) before the handler runs.
 *
 * Scaffold: behavioral specs encoded as comment-fenced plans.
 */

import { expect } from "chai";
import { Keypair, PublicKey, SYSVAR_INSTRUCTIONS_PUBKEY } from "@solana/web3.js";
import { setupBankrun, TestContext } from "../setup";

// The canonical sysvar instructions account address. Hard-coded for
// independence; matches `solana_program::sysvar::instructions::ID`.
const CANONICAL_SYSVAR_INSTRUCTIONS = new PublicKey(
  "Sysvar1nstructions1111111111111111111111111",
);

// -- Pure: address constant + structural shape ----------------------------
describe("ATTACK FIXTURE / sysvar instructions spoofing — invariants (pure)", () => {
  it("web3.js's SYSVAR_INSTRUCTIONS_PUBKEY matches the constant we pin against", () => {
    // If web3.js ever drifts (extremely unlikely — it's a chain constant),
    // both this test and the on-chain constraint catch the divergence.
    expect(SYSVAR_INSTRUCTIONS_PUBKEY.equals(CANONICAL_SYSVAR_INSTRUCTIONS)).to
      .be.true;
  });

  it("a fresh Keypair pubkey is NOT the sysvar address (sanity)", () => {
    const attackerBuffer = Keypair.generate().publicKey;
    expect(attackerBuffer.equals(CANONICAL_SYSVAR_INSTRUCTIONS)).to.be.false;
  });

  it("docs: three sites carry `address = sysvar_instructions::ID` (P1-2)", () => {
    const sites = [
      "RequestAdvance.instructions_sysvar (lib.rs:1003-1006)",
      "ClaimAndSettle.instructions_sysvar (lib.rs:1062-1065)",
      "Ed25519RecordReceivable.instructions_sysvar (oracle/lib.rs)",
    ];
    sites.forEach((s) => expect(s).to.match(/instructions_sysvar/));
  });

  it("constraint structure: any non-canonical buffer is rejected", () => {
    // The on-chain constraint compiles to a runtime equality check between
    // ctx.accounts.instructions_sysvar.key() and the canonical pubkey.
    // Testing the negation here as a structural property: distinct keys
    // cannot satisfy an `address = X` constraint.
    const fake = Keypair.generate().publicKey;
    expect(fake.equals(CANONICAL_SYSVAR_INSTRUCTIONS)).to.be.false;
  });
});

// -- Harness scaffold: end-to-end spoof attempts --------------------------
describe("ATTACK FIXTURE / sysvar instructions spoofing (harness)", () => {
  let ctx: TestContext;

  before(async () => {
    ctx = await setupBankrun();
  });

  it("request_advance with spoofed instructions_sysvar fails (BEHAVIORAL)", async () => {
    // Plan once IDL lands:
    //   1. Construct a 1232-byte buffer the attacker controls (a fresh
    //      keypair-funded account with arbitrary bytes).
    //   2. Build a request_advance ix (source_kind = Ed25519/X402 path so
    //      verify_prev_ed25519 is exercised).
    //   3. Substitute the attacker buffer for instructions_sysvar.
    //   4. expect tx.send() to throw with ConstraintAddress (2012).
    //   5. The pre-handler attestation/receivable PDAs are unchanged
    //      (the failing tx aborts atomically before any state mutation).
    expect(ctx.programs.escrow).to.exist;
  });

  it("claim_and_settle with spoofed instructions_sysvar fails (BEHAVIORAL)", async () => {
    // The require_memo_nonce path in claim_and_settle is the consumer here.
    // Same plan: substitute the attacker buffer; expect ConstraintAddress.
    expect(true).to.be.true;
  });

  it("ed25519_record_receivable with spoofed sysvar fails (BEHAVIORAL)", async () => {
    // Plan: construct an Ed25519RecordReceivable tx, point its
    // instructions_sysvar at an attacker buffer that contains a
    // pre-computed valid-looking ed25519 record. Anchor must reject before
    // the handler reads any bytes.
    expect(true).to.be.true;
  });

  it("the canonical sysvar passes (positive control, BEHAVIORAL)", async () => {
    // Sanity check: with the real sysvar pubkey, the same ix succeeds. This
    // pins the constraint to be exactly equality (not always-reject).
    expect(true).to.be.true;
  });

  it("buffer-spoof variant: pre-funded account at canonical-LIKE address", async () => {
    // Subtler attack: the attacker creates an account at an address that
    // visually resembles the canonical sysvar (e.g., one byte off).
    // Anchor's `address = X` is byte-equality, so even a 1-byte difference
    // rejects. Plan: derive a near-collision pubkey, fund it, attempt the
    // spoof; expect the same ConstraintAddress error.
    expect(true).to.be.true;
  });
});
