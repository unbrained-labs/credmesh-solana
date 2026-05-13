// programs/credmesh-escrow/tests/helpers/ed25519.ts
//
// Build a valid ed25519-signed credit attestation matching the canonical
// 128-byte layout in `crates/credmesh-shared/src/lib.rs::ed25519_credit_message`:
//
//   [0..32)   agent_pubkey
//   [32..64)  pool_pubkey
//   [64..72)  credit_limit_atoms (u64 LE)
//   [72..80)  outstanding_balance (u64 LE)
//   [80..88)  expires_at (i64 LE)
//   [88..96)  attested_at (i64 LE)
//   [96..112) nonce (16 bytes)
//   [112..120) chain_id (u64 LE)
//   [120..128) version (u64 LE) = 1
//
// Also exports a builder for the native ed25519 verify ix in the format
// `credmesh_shared::ix_introspection::verify_prev_ed25519` accepts. The
// verify ix must:
//   - have exactly one signature entry (num_signatures = 1)
//   - have every offset's instruction-index point at the verify ix itself
//     (the asymmetric.re/Relay-class fix in ix_introspection.rs)
//
// `Ed25519Program.createInstructionWithPrivateKey` from web3.js builds an
// ix that satisfies both constraints by default — its layout matches the
// hand-rolled parser in ix_introspection.rs byte-for-byte.

import nacl from "tweetnacl";
import {
  Ed25519Program,
  Keypair,
  PublicKey,
  TransactionInstruction,
} from "@solana/web3.js";

export const MESSAGE_TOTAL_LEN = 128;
export const MESSAGE_VERSION = 1n;

export interface CreditAttestation {
  agent: PublicKey;
  pool: PublicKey;
  creditLimit: bigint;
  outstanding: bigint;
  expiresAt: bigint;
  attestedAt: bigint;
  nonce: Buffer; // exactly 16 bytes
  chainId: bigint;
  version?: bigint; // defaults to 1
}

/// Encode the 128-byte canonical attestation message. Layout is the source
/// of truth in `crates/credmesh-shared/src/lib.rs::ed25519_credit_message`;
/// if you change offsets there, change them here in the same commit (and in
/// `ts/shared/src/index.ts`).
export function encodeCreditMessage(a: CreditAttestation): Buffer {
  if (a.nonce.length !== 16) {
    throw new Error(`nonce must be exactly 16 bytes; got ${a.nonce.length}`);
  }
  const buf = Buffer.alloc(MESSAGE_TOTAL_LEN);
  a.agent.toBuffer().copy(buf, 0); // [0..32)
  a.pool.toBuffer().copy(buf, 32); // [32..64)
  buf.writeBigUInt64LE(a.creditLimit, 64); // [64..72)
  buf.writeBigUInt64LE(a.outstanding, 72); // [72..80)
  buf.writeBigInt64LE(a.expiresAt, 80); // [80..88)
  buf.writeBigInt64LE(a.attestedAt, 88); // [88..96)
  a.nonce.copy(buf, 96); // [96..112)
  buf.writeBigUInt64LE(a.chainId, 112); // [112..120)
  buf.writeBigUInt64LE(a.version ?? MESSAGE_VERSION, 120); // [120..128)
  return buf;
}

/// Sign a credit-attestation message with the given bridge keypair and
/// return both the raw signed bytes and the corresponding `ed25519_program`
/// verify ix that `verify_prev_ed25519` will accept.
///
/// `Ed25519Program.createInstructionWithPrivateKey` produces an ix whose
/// internal offset table references the verify ix itself by index — which
/// is what the on-chain `Ed25519OffsetMismatch` guard requires.
export function signCreditAttestation(
  bridge: Keypair,
  attestation: CreditAttestation,
): { message: Buffer; signature: Buffer; verifyIx: TransactionInstruction } {
  const message = encodeCreditMessage(attestation);
  const signature = Buffer.from(
    nacl.sign.detached(message, bridge.secretKey),
  );
  const verifyIx = Ed25519Program.createInstructionWithPublicKey({
    publicKey: bridge.publicKey.toBytes(),
    message,
    signature,
  });
  return { message, signature, verifyIx };
}
