/**
 * credmesh-solana-bridge entrypoint.
 *
 * HTTP service that signs ed25519 credit attestations for Solana
 * request_advance. EVM is the canonical reputation/identity ledger;
 * the bridge reads it and signs a short-TTL attestation Solana
 * consumes.
 *
 * Quote flow:
 *   1. Agent (or its operator) POST /quote
 *      { agent_pubkey_b58, pool_pubkey_b58, nonce_hex }
 *   2. Bridge:
 *      a. resolves agent_pubkey_b58 → its registered EVM agent
 *         address via the bridge-operator-curated agent-binding map
 *         (loaded from BRIDGE_AGENT_BINDINGS_PATH at startup). Solana
 *         ed25519 keys and EVM secp256k1 addresses do NOT share a key;
 *         the binding is explicit, not derived. An unknown Solana
 *         pubkey is REFUSED — the bridge never signs an attestation
 *         for an agent it can't authoritatively pin to an EVM record.
 *      b. reads (creditLimit, outstanding) from EVM for that
 *         resolved address.
 *      c. encodes a 128-byte ed25519_credit_message.
 *      d. signs with the bridge's ed25519 secret key.
 *      e. returns { message_b64, signature_b64, signer_pubkey_b58, ... }.
 *   3. Agent submits a Solana tx:
 *      [ed25519_verify(...), request_advance(receivable_id, amount, nonce)]
 *      where the ed25519_verify ix references the bridge's signed message.
 *
 * Event tail flow (pending the EVM-side handoff endpoint; not yet
 * wired — see README "Pending the EVM-side handoff endpoint"):
 *   subscribes to Solana escrow logs and replays AdvanceIssued /
 *   AdvanceSettled / AdvanceLiquidated to EVM AgentRecord state.
 */

import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { address as solAddr, getAddressEncoder } from "@solana/kit";
import { encodeAttestation, signAttestation } from "./attestation.js";
import { EvmReader, type EvmConfig } from "./evm.js";

const REQUIRED_ENV = [
  "SOLANA_RPC_URL",
  "EVM_RPC_URL",
  "EVM_REPUTATION_CREDIT_ORACLE_ADDRESS",
  "EVM_TRUSTLESS_ESCROW_ADDRESS",
  "BRIDGE_SIGNING_KEY_PATH",
  "BRIDGE_AGENT_BINDINGS_PATH",
  "SOLANA_ESCROW_PROGRAM_ID",
  "SOLANA_ATTESTOR_REGISTRY_PROGRAM_ID",
  "SOLANA_CHAIN_ID",
] as const;

function loadConfig() {
  for (const k of REQUIRED_ENV) {
    if (!process.env[k]) {
      console.error(`missing required env: ${k}`);
      process.exit(1);
    }
  }
  const evm: EvmConfig = {
    rpcUrl: process.env.EVM_RPC_URL!,
    reputationCreditOracle: process.env.EVM_REPUTATION_CREDIT_ORACLE_ADDRESS! as `0x${string}`,
    trustlessEscrow: process.env.EVM_TRUSTLESS_ESCROW_ADDRESS! as `0x${string}`,
  };
  const solanaChainId = BigInt(process.env.SOLANA_CHAIN_ID!);
  if (solanaChainId !== 1n && solanaChainId !== 2n) {
    console.error(`SOLANA_CHAIN_ID must be 1 (mainnet) or 2 (devnet); got ${solanaChainId}`);
    process.exit(1);
  }
  const signingKey = loadSigningKey(process.env.BRIDGE_SIGNING_KEY_PATH!);
  const agentBindings = loadAgentBindings(process.env.BRIDGE_AGENT_BINDINGS_PATH!);
  return { evm, solanaChainId, signingKey, agentBindings };
}

/**
 * The agent-binding map is the bridge's authoritative Solana → EVM
 * identity table. Each entry pins a Solana ed25519 pubkey to the EVM
 * account whose reputation/credit values may be signed into an
 * attestation for it. The binding is set by the bridge operator (out
 * of band — typically when the agent registers on EVM and declares its
 * Solana key); the bridge NEVER trusts a caller-supplied EVM address.
 *
 * File format: JSON object `{ "<solana_pubkey_b58>": "0x<evm_address>", ... }`
 * — one entry per registered agent. Reloads require a process restart;
 * hot-reload is a v1.5 nice-to-have.
 */
function loadAgentBindings(path: string): Map<string, `0x${string}`> {
  const raw = readFileSync(path, "utf-8");
  const parsed = JSON.parse(raw) as Record<string, string>;
  const map = new Map<string, `0x${string}`>();
  for (const [solana, evm] of Object.entries(parsed)) {
    if (!/^0x[0-9a-fA-F]{40}$/.test(evm)) {
      throw new Error(
        `agent binding ${solana} → ${evm} is not a 20-byte EVM address`,
      );
    }
    map.set(solana, evm.toLowerCase() as `0x${string}`);
  }
  if (map.size === 0) {
    throw new Error(
      `agent bindings file at ${path} is empty — refusing to start, the bridge would have no agents to attest for`,
    );
  }
  return map;
}

function loadSigningKey(path: string): Uint8Array {
  const raw = readFileSync(path, "utf-8");
  const bytes = Uint8Array.from(JSON.parse(raw));
  if (bytes.length !== 64) {
    throw new Error(
      `bridge signing key must be a 64-byte ed25519 keypair (Solana keypair JSON format); got ${bytes.length} bytes`,
    );
  }
  return bytes;
}

interface QuoteRequest {
  agent_pubkey_b58: string;
  pool_pubkey_b58: string;
  nonce_hex: string;
  ttl_seconds?: number;
}

interface QuoteResponse {
  message_b64: string;
  signature_b64: string;
  signer_pubkey_b58: string;
  expires_at: number;
  attested_at: number;
  credit_limit_atoms: string;
  outstanding_atoms: string;
}

async function handleQuote(
  body: QuoteRequest,
  cfg: ReturnType<typeof loadConfig>,
): Promise<QuoteResponse> {
  const ttlSeconds = Math.min(body.ttl_seconds ?? 600, 15 * 60);
  if (ttlSeconds <= 0) throw new Error("ttl_seconds must be positive");

  const agentEncoder = getAddressEncoder();
  const agentBytes = agentEncoder.encode(solAddr(body.agent_pubkey_b58));
  const poolBytes = agentEncoder.encode(solAddr(body.pool_pubkey_b58));

  if (body.nonce_hex.length !== 32 && body.nonce_hex.length !== 34) {
    throw new Error("nonce_hex must encode 16 bytes (32 hex chars, optionally 0x-prefixed)");
  }
  const nonceHex = body.nonce_hex.startsWith("0x") ? body.nonce_hex.slice(2) : body.nonce_hex;
  const nonce = new Uint8Array(
    nonceHex.match(/.{1,2}/g)!.map((b) => parseInt(b, 16)),
  );

  // Resolve the EVM address authoritatively from the bridge-operator-
  // curated binding map. The bridge NEVER trusts a caller-supplied EVM
  // address — that would let any caller request a signed attestation
  // binding their Solana key to an unrelated EVM whale's credit limit.
  const evmAddress = cfg.agentBindings.get(body.agent_pubkey_b58);
  if (!evmAddress) {
    throw new Error(
      `agent ${body.agent_pubkey_b58} is not registered in the bridge agent-binding map`,
    );
  }

  const evm = new EvmReader(cfg.evm);
  const snap = await evm.fetchAgent(evmAddress);
  if (snap.creditLimitAtoms === 0n) {
    throw new Error(
      "agent has zero credit limit on EVM (either MIN_CREDIT_SCORE not met, agent not registered, or quarantined)",
    );
  }

  const now = BigInt(Math.floor(Date.now() / 1000));
  const expiresAt = now + BigInt(ttlSeconds);
  const message = encodeAttestation({
    agentPubkey: agentBytes as Uint8Array,
    poolPubkey: poolBytes as Uint8Array,
    creditLimitAtoms: snap.creditLimitAtoms,
    outstandingAtoms: snap.outstandingAtoms,
    expiresAt,
    attestedAt: now,
    nonce,
    chainId: cfg.solanaChainId,
  });
  const signature = signAttestation(message, cfg.signingKey);

  const signerPub = cfg.signingKey.slice(32);
  return {
    message_b64: Buffer.from(message).toString("base64"),
    signature_b64: Buffer.from(signature).toString("base64"),
    signer_pubkey_b58: bytesToBase58(signerPub),
    expires_at: Number(expiresAt),
    attested_at: Number(now),
    credit_limit_atoms: snap.creditLimitAtoms.toString(),
    outstanding_atoms: snap.outstandingAtoms.toString(),
  };
}

function bytesToBase58(bytes: Uint8Array): string {
  // Minimal base58 encoder (Solana alphabet) — keeps the dependency
  // surface small. For a 32-byte pubkey this is fine; not used on hot
  // paths.
  const ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  let zeros = 0;
  for (const b of bytes) {
    if (b === 0) zeros += 1;
    else break;
  }
  const digits = [0];
  for (let i = zeros; i < bytes.length; i++) {
    let carry = bytes[i];
    for (let j = 0; j < digits.length; j++) {
      carry += digits[j] << 8;
      digits[j] = carry % 58;
      carry = (carry / 58) | 0;
    }
    while (carry > 0) {
      digits.push(carry % 58);
      carry = (carry / 58) | 0;
    }
  }
  let out = "";
  for (let i = 0; i < zeros; i++) out += "1";
  for (let i = digits.length - 1; i >= 0; i--) out += ALPHABET[digits[i]];
  return out;
}

async function main() {
  const cfg = loadConfig();
  const port = Number(process.env.BRIDGE_PORT ?? 4001);

  const server = createServer(async (req, res) => {
    if (req.method === "GET" && req.url === "/health") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(JSON.stringify({ ok: true }));
      return;
    }
    if (req.method === "POST" && req.url === "/quote") {
      try {
        const chunks: Buffer[] = [];
        for await (const chunk of req) chunks.push(chunk as Buffer);
        const body = JSON.parse(Buffer.concat(chunks).toString()) as QuoteRequest;
        const reply = await handleQuote(body, cfg);
        res.writeHead(200, { "Content-Type": "application/json" });
        res.end(JSON.stringify(reply));
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        res.writeHead(400, { "Content-Type": "application/json" });
        res.end(JSON.stringify({ error: msg }));
      }
      return;
    }
    res.writeHead(404, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ error: "not found" }));
  });

  server.listen(port, () => {
    console.log(`credmesh-solana-bridge listening on :${port}`);
    console.log(`  EVM:               ${cfg.evm.rpcUrl}`);
    console.log(`  Solana chain id:   ${cfg.solanaChainId}`);
    console.log(`  Agents registered: ${cfg.agentBindings.size}`);
    console.log(`  Signer pubkey:     ${bytesToBase58(cfg.signingKey.slice(32))}`);
  });
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
