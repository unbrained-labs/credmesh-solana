// programs/credmesh-escrow/tests/request_advance.test.ts
//
// Adversarial-fixture proof-of-concept for the bankrun harness. One test:
//
//   T-CRY-08: cross-agent ed25519 replay
//   ──────────────────────────────────
//   The bridge produces a credit attestation valid for `agent_a`. Agent B
//   captures the attestation (e.g., from a public mempool or a logged
//   tx-build payload) and tries to consume it as their own borrow. The
//   on-chain handler MUST reject with `Ed25519MessageMismatch` because
//   `msg.agent_pubkey != ctx.accounts.agent.key()`.
//
//   This is the canonical sanity check on the "attestation binds to one
//   agent" invariant in `crates/credmesh-shared/src/lib.rs`'s
//   `ed25519_credit_message` layout — without it, a single compromised
//   bridge signature would drain credit lines for every other agent on
//   the pool.

import { createHash } from "node:crypto";
import { expect } from "chai";
import {
  ComputeBudgetProgram,
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";

import { bootstrap, CHAIN_ID_DEVNET, ESCROW_PROGRAM_ID, TestCtx } from "./helpers/setup";
import { signCreditAttestation } from "./helpers/ed25519";

const ADVANCE_SEED = Buffer.from("advance");
const CONSUMED_SEED = Buffer.from("consumed");
const ISSUANCE_LEDGER_SEED = Buffer.from("issuance_ledger");

function anchorDiscriminator(method: string): Buffer {
  return createHash("sha256")
    .update(`global:${method}`)
    .digest()
    .subarray(0, 8);
}

/// Anchor account discriminator: first 8 bytes of sha256("account:<TypeName>").
function accountDiscriminator(typeName: string): Buffer {
  return createHash("sha256")
    .update(`account:${typeName}`)
    .digest()
    .subarray(0, 8);
}

/// Hand-encode `request_advance(receivable_id, amount, nonce)`:
///   discriminator(8) || receivable_id(32) || amount(u64 LE) || nonce(16) = 64 bytes
function encodeRequestAdvance(
  receivableId: Buffer,
  amount: bigint,
  nonce: Buffer,
): Buffer {
  if (receivableId.length !== 32) throw new Error("receivable_id must be 32B");
  if (nonce.length !== 16) throw new Error("nonce must be 16B");
  const buf = Buffer.alloc(8 + 32 + 8 + 16);
  anchorDiscriminator("request_advance").copy(buf, 0);
  receivableId.copy(buf, 8);
  buf.writeBigUInt64LE(amount, 40);
  nonce.copy(buf, 48);
  return buf;
}

/// Pre-stamp an `AllowedSigner` PDA at the correct address via bankrun's raw
/// `setAccount` API. Sidesteps the Squads CPI gate on `add_allowed_signer`
/// — useful for tests that need a whitelisted bridge without simulating a
/// multisig flow. Layout from
/// `programs/credmesh-attestor-registry/src/state.rs::AllowedSigner`:
///
///   discriminator(8) || bump(u8) || signer(Pubkey 32) || kind(u8) || added_at(i64)
///   = 50 bytes total
function prestampAllowedSigner(
  ctx: TestCtx,
  bridgePubkey: PublicKey,
  bump: number,
): PublicKey {
  const pda = ctx.derivedAllowedSignerPda(bridgePubkey);
  const data = Buffer.alloc(50);
  accountDiscriminator("AllowedSigner").copy(data, 0);
  data.writeUInt8(bump, 8);
  bridgePubkey.toBuffer().copy(data, 9);
  data.writeUInt8(0, 41); // kind = 0 (CreditBridge)
  data.writeBigInt64LE(0n, 42); // added_at = 0 (audit-trail only)
  ctx.context.setAccount(pda, {
    lamports: 1_000_000_000, // rent-exempt-by-default; bankrun is lenient
    data,
    owner: new PublicKey("ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk"),
    executable: false,
  });
  return pda;
}

function derivePda(seeds: Buffer[], programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(seeds, programId);
}

describe("request_advance — adversarial", () => {
  let ctx: TestCtx;

  before(async function () {
    this.timeout(60_000);
    ctx = await bootstrap();
  });

  it("T-CRY-08: rejects cross-agent ed25519 replay (msg.agent != ctx.agent)", async () => {
    // (1) Bridge keypair + whitelist via raw setAccount.
    const bridge = Keypair.generate();
    const [, allowedBump] = derivePda(
      [Buffer.from("allowed_signer"), bridge.publicKey.toBuffer()],
      new PublicKey("ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk"),
    );
    prestampAllowedSigner(ctx, bridge.publicKey, allowedBump);

    // (2) Bridge signs an attestation BOUND TO AGENT A. Agent A's pubkey
    // sits in the `agent_pubkey` field of the canonical 128-byte msg.
    const nonce = Buffer.from("00112233445566778899aabbccddeeff", "hex");
    const receivableId = Buffer.alloc(32);
    receivableId.write("cross-agent-replay-T-CRY-08", "utf8");

    // attested_at must be within MAX_ATTESTATION_AGE_SECONDS of `now`
    // (bankrun's clock starts at the host's unix-ts, so `Date.now()` works).
    const nowSec = BigInt(Math.floor(Date.now() / 1000));
    const { verifyIx } = signCreditAttestation(bridge, {
      agent: ctx.agentA.publicKey, // ← attestation says: this credit is for AGENT A
      pool: ctx.poolPda,
      creditLimit: 100_000_000n,
      outstanding: 0n,
      expiresAt: nowSec + 600n,
      attestedAt: nowSec,
      nonce,
      chainId: CHAIN_ID_DEVNET,
    });

    // (3) Build a `request_advance` ix WHERE THE SIGNING AGENT IS AGENT B.
    // The handler's `require_keys_eq!(msg_agent, ctx.accounts.agent.key(),
    // Ed25519MessageMismatch)` should trip.
    const advance = derivePda(
      [
        ADVANCE_SEED,
        ctx.poolPda.toBuffer(),
        ctx.agentB.publicKey.toBuffer(),
        receivableId,
      ],
      ESCROW_PROGRAM_ID,
    )[0];
    const consumed = derivePda(
      [
        CONSUMED_SEED,
        ctx.poolPda.toBuffer(),
        ctx.agentB.publicKey.toBuffer(),
        receivableId,
      ],
      ESCROW_PROGRAM_ID,
    )[0];
    const issuanceLedger = derivePda(
      [
        ISSUANCE_LEDGER_SEED,
        ctx.poolPda.toBuffer(),
        ctx.agentB.publicKey.toBuffer(),
      ],
      ESCROW_PROGRAM_ID,
    )[0];
    const allowedSigner = ctx.derivedAllowedSignerPda(bridge.publicKey);

    const requestIx = new TransactionInstruction({
      programId: ESCROW_PROGRAM_ID,
      keys: [
        { pubkey: ctx.agentB.publicKey, isSigner: true, isWritable: true },
        { pubkey: allowedSigner, isSigner: false, isWritable: false },
        { pubkey: ctx.poolPda, isSigner: false, isWritable: true },
        { pubkey: advance, isSigner: false, isWritable: true },
        { pubkey: consumed, isSigner: false, isWritable: true },
        { pubkey: issuanceLedger, isSigner: false, isWritable: true },
        { pubkey: ctx.poolUsdcVault, isSigner: false, isWritable: true },
        { pubkey: ctx.agentBUsdcAta, isSigner: false, isWritable: true },
        { pubkey: ctx.usdcMint, isSigner: false, isWritable: false },
        { pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false },
        { pubkey: TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data: encodeRequestAdvance(receivableId, 1_000_000n, nonce),
    });

    // (4) Tx layout: ed25519 verify ix MUST be IMMEDIATELY before
    // `request_advance` (handler reads `cur_idx - 1`). A compute-budget ix
    // would slot AFTER request_advance in this order to avoid breaking the
    // adjacency invariant.
    const tx = new Transaction();
    tx.add(verifyIx);
    tx.add(requestIx);
    tx.add(ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 }));
    tx.recentBlockhash = ctx.context.lastBlockhash;
    tx.feePayer = ctx.agentB.publicKey;
    tx.sign(ctx.agentB);

    let threw = false;
    let errString = "";
    try {
      await ctx.banksClient.processTransaction(tx);
    } catch (e) {
      threw = true;
      errString = (e as Error).message ?? String(e);
    }
    expect(threw, "tx must revert").to.equal(true);
    // The exact format of bankrun error strings is
    // "TransactionError ... custom program error: 0x<hex>" where the hex is
    // the Anchor-mapped error code. Ed25519MessageMismatch is the 9th
    // variant in `CredmeshError` (0-indexed = 8), so the on-chain code is
    // 6000 + 8 = 6008 = 0x1778.
    //
    // We assert the substring matches to keep the test robust against
    // BanksClient's logging-format variance across solana-bankrun
    // releases. The semantic assertion is the same: this specific error
    // code, not any-old revert.
    expect(errString.toLowerCase()).to.match(
      /0x1778|ed25519messagemismatch/i,
      `expected Ed25519MessageMismatch (0x1778); got: ${errString}`,
    );
  });
});
