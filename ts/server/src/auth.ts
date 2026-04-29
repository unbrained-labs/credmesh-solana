/**
 * SIWS (Sign-In With Solana) auth middleware.
 *
 * Per DESIGN §6 and DECISIONS Q8. Replaces EIP-191 personal_sign with the
 * Solana-native SIWS pattern (CAIP-122). Phantom v23.11+, Solflare, and
 * Backpack all render SIWS messages with anti-phishing UI when they detect
 * the canonical format.
 *
 * Headers (from credit-worker EIP-191 → SIWS port):
 *   X-Agent-Address    base58-encoded ed25519 pubkey (32 bytes)
 *   X-Agent-Signature  base58-encoded detached ed25519 signature (64 bytes)
 *   X-Agent-Timestamp  ISO 8601 — must equal `Issued At` in the SIWS message
 *   X-Agent-Cluster    "mainnet-beta" | "devnet" — prevents cross-cluster replay
 *   X-Agent-Nonce      server-issued nonce; matched against a one-shot store
 *
 * The signed payload is the canonical SIWS string, NOT just a short prefix.
 * Wallets warn users about non-SIWS messages.
 */

import { createMiddleware } from "hono/factory";
import nacl from "tweetnacl";
import bs58 from "bs58";

const SIWS_DOMAIN = process.env.CREDMESH_DOMAIN ?? "credmesh.xyz";
const TIMESTAMP_TOLERANCE_MS = 10 * 60 * 1000; // ±10 min
const NONCE_TTL_MS = 5 * 60 * 1000;
const NONCE_GC_PROBABILITY = 0.05;
const NONCE_MAX_SIZE = 100_000;

// CAIP-2 chain IDs (genesis hash prefixes). Required for Phantom anti-phishing UI.
const CAIP2_CHAIN_ID: Record<"mainnet-beta" | "devnet", string> = {
  "mainnet-beta": "solana:5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp",
  devnet: "solana:EtWTRABZaYq6iMfeYKouRu166VU2xqa1S5jJUrdiJzMv",
};

export interface SiwsPayload {
  domain: string;
  address: string;
  statement: string;
  uri: string;
  version: string;
  chainId: string;
  nonce: string;
  issuedAt: string;
  expirationTime: string;
  resources?: string[];
}

export function buildSiwsMessage(p: SiwsPayload): string {
  const lines = [
    `${p.domain} wants you to sign in with your Solana account:`,
    p.address,
    "",
    p.statement,
    "",
    `URI: ${p.uri}`,
    `Version: ${p.version}`,
    `Chain ID: ${p.chainId}`,
    `Nonce: ${p.nonce}`,
    `Issued At: ${p.issuedAt}`,
    `Expiration Time: ${p.expirationTime}`,
  ];
  if (p.resources && p.resources.length > 0) {
    lines.push("Resources:");
    for (const r of p.resources) lines.push(`- ${r}`);
  }
  return lines.join("\n");
}

class NonceStore {
  private nonces = new Map<string, { issuedAt: number; address: string }>();

  issue(address: string): string {
    if (this.nonces.size >= NONCE_MAX_SIZE || Math.random() < NONCE_GC_PROBABILITY) {
      this.gc();
    }
    const nonce = randomNonce(16);
    this.nonces.set(nonce, { issuedAt: Date.now(), address });
    return nonce;
  }

  consume(nonce: string, expectedAddress: string): boolean {
    const entry = this.nonces.get(nonce);
    if (!entry) return false;
    if (entry.address !== expectedAddress) return false;
    if (Date.now() - entry.issuedAt > NONCE_TTL_MS) {
      this.nonces.delete(nonce);
      return false;
    }
    this.nonces.delete(nonce);
    return true;
  }

  // Map preserves insertion order; iterate from the front and break on the
  // first entry that's still within TTL.
  private gc(): void {
    const cutoff = Date.now() - NONCE_TTL_MS;
    for (const [nonce, entry] of this.nonces) {
      if (entry.issuedAt > cutoff) break;
      this.nonces.delete(nonce);
    }
  }
}

const nonceStore = new NonceStore();

export function issueNonce(address: string): string {
  return nonceStore.issue(address);
}

function randomNonce(byteLen: number): string {
  const bytes = new Uint8Array(byteLen);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

export interface AuthContext {
  verifiedAddress: string;
  cluster: "mainnet-beta" | "devnet";
}

export const authMiddleware = createMiddleware<{
  Variables: { auth: AuthContext };
}>(async (c, next) => {
  const method = c.req.method;
  if (method === "GET" || method === "HEAD" || method === "OPTIONS") {
    await next();
    return;
  }

  const address = c.req.header("X-Agent-Address");
  const signature = c.req.header("X-Agent-Signature");
  const timestamp = c.req.header("X-Agent-Timestamp");
  const cluster = c.req.header("X-Agent-Cluster") as "mainnet-beta" | "devnet" | undefined;
  const nonce = c.req.header("X-Agent-Nonce");
  const requestUri = c.req.url;

  if (!address || !signature || !timestamp || !cluster || !nonce) {
    return c.json({ error: "missing SIWS auth headers" }, 401);
  }

  if (cluster !== "mainnet-beta" && cluster !== "devnet") {
    return c.json({ error: "invalid X-Agent-Cluster" }, 401);
  }

  const issuedAt = Date.parse(timestamp);
  if (Number.isNaN(issuedAt) || Math.abs(Date.now() - issuedAt) > TIMESTAMP_TOLERANCE_MS) {
    return c.json({ error: "timestamp out of tolerance" }, 401);
  }

  if (!nonceStore.consume(nonce, address)) {
    return c.json({ error: "invalid or expired nonce" }, 401);
  }

  const expirationTime = new Date(issuedAt + 5 * 60 * 1000).toISOString();
  const expectedMessage = buildSiwsMessage({
    domain: SIWS_DOMAIN,
    address,
    statement: "Authenticate to CredMesh",
    uri: requestUri,
    version: "1",
    chainId: CAIP2_CHAIN_ID[cluster],
    nonce,
    issuedAt: timestamp,
    expirationTime,
  });

  let pubkeyBytes: Uint8Array;
  let sigBytes: Uint8Array;
  try {
    pubkeyBytes = bs58.decode(address);
    sigBytes = bs58.decode(signature);
  } catch {
    return c.json({ error: "malformed base58 in headers" }, 401);
  }

  if (pubkeyBytes.length !== 32) return c.json({ error: "invalid pubkey length" }, 401);
  if (sigBytes.length !== 64) return c.json({ error: "invalid signature length" }, 401);

  const messageBytes = new TextEncoder().encode(expectedMessage);
  const valid = nacl.sign.detached.verify(messageBytes, sigBytes, pubkeyBytes);
  if (!valid) return c.json({ error: "signature verification failed" }, 401);

  c.set("auth", { verifiedAddress: address, cluster });
  await next();
});

export function assertAuthorized(verified: string, target: string): void {
  if (verified !== target) {
    throw new Error(`unauthorized: signer ${verified} cannot act for ${target}`);
  }
}
