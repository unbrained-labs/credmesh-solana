/**
 * Single-source-of-truth TS constants kept in sync with Rust.
 *
 * Rule: every constant exported here has its Rust counterpart cited.
 * If you change a value, update both. EVM-parity tests assert these
 * match `../trustvault-credit/packages/protocol-spec/index.js`.
 */

export const BPS_DENOMINATOR = 10_000;

// ── Source kinds and claim ratios ──────────────────────────────────────────
//
// Mirror of `crates/credmesh-shared/src/lib.rs::SourceKind`.
// Mirror of EVM `packages/protocol-spec/index.js::CLAIM_RATIOS`.
//
// Worker / Marketplace map to EVM `worker_attested` (10%);
// Ed25519 / X402 map to EVM `signed_receivable` (20%);
// EVM `venue_state` (30%) is intentionally not on Solana v1.

export const SOURCE_KIND = {
  Worker: 0,
  Ed25519: 1,
  X402: 2,
  Marketplace: 3,
} as const;
export type SourceKindName = keyof typeof SOURCE_KIND;
export type SourceKindByte = (typeof SOURCE_KIND)[SourceKindName];

export const CLAIM_RATIO_BPS: Record<SourceKindName, number> = {
  Worker: 1000,
  Marketplace: 1000,
  Ed25519: 2000,
  X402: 2000,
};

// EVM equivalents (for parity-test assertions).
export const EVM_CLAIM_RATIOS = {
  worker_attested: 0.1,
  signed_receivable: 0.2,
  venue_state: 0.3,
} as const;

// ── Reputation / scoring constants ─────────────────────────────────────────
//
// Mirror of `programs/credmesh-reputation/src/state.rs`.

export const ATOMS_PER_USDC = 1_000_000n;
export const MAX_CREDIT_LIMIT_USD = 1_000n;
export const MAX_CREDIT_LIMIT_ATOMS = MAX_CREDIT_LIMIT_USD * ATOMS_PER_USDC;

// Used by the score formula and `compute_credit_score` saturation caps.
export const SCORE_OUTSTANDING_CAP_USD = 100n;
export const SCORE_AVG_PAYOUT_CAP_USD = 200n;

// ── Escrow PDA seeds ───────────────────────────────────────────────────────
//
// Mirror of `crates/credmesh-shared/src/seeds.rs`. Used by ts/keeper for
// PDA derivation; the Rust side reads these via `pub use`.

export const POOL_SEED = "pool";
export const ADVANCE_SEED = "advance";
export const CONSUMED_SEED = "consumed";
export const TREASURY_SEED = "treasury";
export const REPUTATION_SEED = "agent_reputation";
export const RECEIVABLE_SEED = "receivable";
export const ALLOWED_SIGNER_SEED = "allowed_signer";
export const ORACLE_CONFIG_SEED = "oracle_config";

// ── Time constants ──────────────────────────────────────────────────────────
//
// Mirror of `programs/credmesh-escrow/src/state.rs`.

export const CLAIM_WINDOW_SECONDS = 7 * 24 * 60 * 60;
export const LIQUIDATION_GRACE_SECONDS = 14 * 24 * 60 * 60;
export const MAX_LATE_DAYS = 365;

// ── Anchor account discriminators ──────────────────────────────────────────
//
// Helper for Anchor account discriminators — first 8 bytes of
// `sha256("account:Foo")`. Used by ts/keeper getProgramAccounts memcmp
// filters and by the on-chain decoder modules.

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
