/**
 * Cross-package TS constants kept in sync with Rust.
 *
 * Rule: every constant exported here has its Rust counterpart cited.
 * If you change a value, update both. The constants are minimal —
 * Solana on-chain logic is now consumer-only (EVM is source of truth
 * for reputation / scoring), so this file is much smaller than it was
 * pre-pivot.
 */

export const BPS_DENOMINATOR = 10_000;
export const ATOMS_PER_USDC = 1_000_000n;

// ── Escrow PDA seeds (mirror crates/credmesh-shared/src/lib.rs::seeds) ─────

export const POOL_SEED = "pool";
export const ADVANCE_SEED = "advance";
export const CONSUMED_SEED = "consumed";
export const ALLOWED_SIGNER_SEED = "allowed_signer";
export const ATTESTOR_CONFIG_SEED = "attestor_config";

// ── Time constants (mirror programs/credmesh-escrow/src/state.rs) ──────────

export const CLAIM_WINDOW_SECONDS = 7 * 24 * 60 * 60;
export const LIQUIDATION_GRACE_SECONDS = 14 * 24 * 60 * 60;
export const MAX_LATE_DAYS = 365;

// ── Bridge ed25519 credit-attestation message format ───────────────────────
// Mirror of crates/credmesh-shared/src/lib.rs::ed25519_credit_message.

export const ED25519_CREDIT_MSG_VERSION = 1n;
export const ED25519_CREDIT_MSG_LEN = 128;

export const ED25519_CREDIT_MSG_OFFSETS = {
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

export const MAX_ATTESTATION_AGE_SECONDS = 15 * 60;

export const SOLANA_CHAIN_ID_MAINNET = 1n;
export const SOLANA_CHAIN_ID_DEVNET = 2n;

// ── Attestor kinds (mirror crates/credmesh-shared/src/lib.rs::AttestorKind) ─

export const ATTESTOR_KIND = {
  CreditBridge: 0,
} as const;
export type AttestorKindName = keyof typeof ATTESTOR_KIND;
export type AttestorKindByte = (typeof ATTESTOR_KIND)[AttestorKindName];

// ── Anchor account / ix discriminator helpers ──────────────────────────────

export async function anchorAccountDiscriminator(
  accountName: string,
): Promise<Uint8Array> {
  const data = new TextEncoder().encode(`account:${accountName}`);
  const buf = await crypto.subtle.digest("SHA-256", data);
  return new Uint8Array(buf).slice(0, 8);
}

export async function anchorIxDiscriminator(
  ixName: string,
): Promise<Uint8Array> {
  const data = new TextEncoder().encode(`global:${ixName}`);
  const buf = await crypto.subtle.digest("SHA-256", data);
  return new Uint8Array(buf).slice(0, 8);
}

/// Anchor `emit!` event discriminator: first 8 bytes of
/// sha256(`event:<EventName>`). The bridge event tail uses this to
/// recognise AdvanceIssued / AdvanceSettled / AdvanceLiquidated
/// records inside the "Program data: <base64>" log lines.
export async function anchorEventDiscriminator(
  eventName: string,
): Promise<Uint8Array> {
  const data = new TextEncoder().encode(`event:${eventName}`);
  const buf = await crypto.subtle.digest("SHA-256", data);
  return new Uint8Array(buf).slice(0, 8);
}
