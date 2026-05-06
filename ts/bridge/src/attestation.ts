/**
 * Canonical 128-byte ed25519 credit attestation builder + signer.
 *
 * Layout MUST match crates/credmesh-shared/src/lib.rs::ed25519_credit_message
 * exactly. Drift = on-chain rejection.
 */

import nacl from "tweetnacl";

export const ED25519_CREDIT_MSG_LEN = 128;
export const ED25519_CREDIT_MSG_VERSION = 1n;
export const MAX_ATTESTATION_AGE_SECONDS = 15 * 60;

const OFFSET = {
  agent: 0,
  pool: 32,
  creditLimit: 64,
  outstanding: 72,
  expiresAt: 80,
  attestedAt: 88,
  nonce: 96,
  chainId: 112,
  version: 120,
} as const;

export interface CreditAttestationInput {
  agentPubkey: Uint8Array;       // 32 bytes
  poolPubkey: Uint8Array;        // 32 bytes
  creditLimitAtoms: bigint;
  outstandingAtoms: bigint;
  expiresAt: bigint;             // unix-seconds
  attestedAt: bigint;            // unix-seconds
  nonce: Uint8Array;             // 16 bytes
  chainId: bigint;               // 1=mainnet, 2=devnet (matches Rust constants)
}

export function encodeAttestation(input: CreditAttestationInput): Uint8Array {
  if (input.agentPubkey.length !== 32) throw new Error("agent must be 32 bytes");
  if (input.poolPubkey.length !== 32) throw new Error("pool must be 32 bytes");
  if (input.nonce.length !== 16) throw new Error("nonce must be 16 bytes");

  const buf = new Uint8Array(ED25519_CREDIT_MSG_LEN);
  buf.set(input.agentPubkey, OFFSET.agent);
  buf.set(input.poolPubkey, OFFSET.pool);
  writeU64LE(buf, OFFSET.creditLimit, input.creditLimitAtoms);
  writeU64LE(buf, OFFSET.outstanding, input.outstandingAtoms);
  writeI64LE(buf, OFFSET.expiresAt, input.expiresAt);
  writeI64LE(buf, OFFSET.attestedAt, input.attestedAt);
  buf.set(input.nonce, OFFSET.nonce);
  writeU64LE(buf, OFFSET.chainId, input.chainId);
  writeU64LE(buf, OFFSET.version, ED25519_CREDIT_MSG_VERSION);
  return buf;
}

export function signAttestation(
  message: Uint8Array,
  signingSecretKey: Uint8Array,
): Uint8Array {
  if (signingSecretKey.length !== 64) {
    throw new Error("ed25519 signing key must be 64-byte (secret + public)");
  }
  return nacl.sign.detached(message, signingSecretKey);
}

export function verifyAttestation(
  message: Uint8Array,
  signature: Uint8Array,
  publicKey: Uint8Array,
): boolean {
  return nacl.sign.detached.verify(message, signature, publicKey);
}

function writeU64LE(buf: Uint8Array, offset: number, value: bigint): void {
  if (value < 0n || value > 0xffffffffffffffffn) {
    throw new Error(`u64 out of range: ${value}`);
  }
  const view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  view.setBigUint64(offset, value, true);
}

function writeI64LE(buf: Uint8Array, offset: number, value: bigint): void {
  if (value < -(2n ** 63n) || value >= 2n ** 63n) {
    throw new Error(`i64 out of range: ${value}`);
  }
  const view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  view.setBigInt64(offset, value, true);
}
