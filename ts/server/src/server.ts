/**
 * CredMesh Solana — backend server.
 *
 * Hono on Node (not Cloudflare Workers, despite EVM repo's "credit-worker" name).
 * Mirrors the EVM credmesh credit-worker structure; DESIGN §6 has the diff.
 *
 * Routes mounted under /agents, /credit, /marketplace, /spend, /treasury,
 * /mandates require SIWS auth on POST/PUT/DELETE; GET passes through.
 */

import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { authMiddleware, issueNonce } from "./auth.js";

const app = new Hono();

app.use(
  "*",
  cors({
    origin: ["https://credmesh.xyz", "https://www.credmesh.xyz", "http://localhost:5173"],
    credentials: true,
  }),
);

app.get("/", (c) =>
  c.json({
    name: "CredMesh Solana",
    version: "0.0.1",
    status: "pre-implementation",
    docs: "https://github.com/unbrained-labs/credmesh-solana",
  }),
);

app.get("/health", (c) => c.json({ ok: true }));

/**
 * A2A agent card. Consumed by the cross-lane outreach agent
 * (`../trustvault-credit/packages/outreach-agent`) which scans DeFi vaults
 * for underperforming yield, discovers their operator's A2A endpoint, and
 * pitches CredMesh as a higher-yield deposit target. The outreach agent
 * fetches this endpoint at runtime; if the `outreach` block is missing or
 * misconfigured, it refuses to run (per its README, no hardcoded contract
 * addresses in that package).
 *
 * Required env (set on the deployed worker):
 *   OUTREACH_CHAIN_ID            e.g. "solana-devnet" or "solana-mainnet"
 *   OUTREACH_VAULT_ADDRESS       Pool PDA (the share-mint vault address)
 *   OUTREACH_EXPLORER_BASE       e.g. "https://solscan.io" (optional)
 *   OUTREACH_MCP_PACKAGE         optional MCP server pkg name for the SDK
 *   OUTREACH_SOURCE_REPO         optional, default the GitHub URL
 *
 * Missing OUTREACH_VAULT_ADDRESS or OUTREACH_CHAIN_ID -> the outreach
 * block is omitted; the outreach agent will treat us as not-ready.
 */
/**
 * Agent card. Built ONCE at module load from env (env is fixed at
 * startup) — saves the per-request object allocation + ~5-10µs latency.
 */
const AGENT_CARD = buildAgentCard();
app.get("/.well-known/agent.json", (c) => c.json(AGENT_CARD));

function buildAgentCard(): Record<string, unknown> {
  const apiBase = process.env.PUBLIC_API_BASE ?? "https://credmesh.xyz";
  const card: Record<string, unknown> = {
    name: "CredMesh Solana",
    description:
      "Revenue-backed working capital for autonomous agents on Solana. Standing credit line, automatic repayment from job revenue, permissionless settlement.",
    a2a: { endpoint: `${apiBase}/agents`, version: "0.1" },
    capabilities: [
      "agent-onboarding",
      "credit-quote",
      "credit-advance",
      "marketplace-job-post",
      "permissionless-settlement",
    ],
  };
  const outreach = buildOutreachBlock(apiBase);
  if (outreach) card.outreach = outreach;
  return card;
}

function buildOutreachBlock(apiBase: string): Record<string, unknown> | null {
  const chainId = process.env.OUTREACH_CHAIN_ID;
  const vaultAddress = process.env.OUTREACH_VAULT_ADDRESS;
  if (!chainId || !vaultAddress) return null;

  const explorerBase = process.env.OUTREACH_EXPLORER_BASE ?? "https://solscan.io";
  return {
    chain: chainId,
    vaultAddress,
    explorerBase,
    explorerUrl: `${explorerBase}/account/${vaultAddress}`,
    apiBase,
    mcpPackage: process.env.OUTREACH_MCP_PACKAGE ?? "@credmesh/mcp-solana",
    sourceRepo:
      process.env.OUTREACH_SOURCE_REPO ??
      "https://github.com/unbrained-labs/credmesh-solana",
    pitch: {
      headline:
        "Stop earning passive yield on idle USDC. Underwrite autonomous agents.",
      body: [
        "CredMesh-Solana underwrites short-duration advances against",
        "marketplace job receivables. Your USDC sits in a Pool PDA;",
        "agents draw against it within their on-chain credit limit;",
        "the protocol clips a 15% fee on every settlement and routes",
        "principal + 85% of fees back to LPs. Permissionless settlement",
        "via SPL Approve delegate (DECISIONS Q9). Three-key topology",
        "(fee-payer / oracle worker / reputation writer) and Squads-",
        "governed FeeCurve updates with timelock.",
      ].join(" "),
      targetMetrics: { minTvlUsd: 50_000, maxApr: 0.06 },
    },
  };
}

app.post("/auth/nonce", async (c) => {
  const body = await c.req.json().catch(() => ({}));
  const address = body.address;
  if (typeof address !== "string" || address.length === 0) {
    return c.json({ error: "address required" }, 400);
  }
  return c.json({ nonce: issueNonce(address) });
});

const writeMounts = ["/agents/*", "/credit/*", "/marketplace/*", "/spend/*", "/treasury/*", "/mandates/*"];
for (const m of writeMounts) {
  app.use(m, authMiddleware);
}

// Routes below are explicitly NOT implemented. Each returns 501 with a
// machine-readable `unblocker` reference so a calling agent (or dashboard)
// can detect-and-route around them rather than getting silent placeholder
// JSON. The unblocker for everything that needs a typed program client is
// issue #15 (Anchor 0.30 IDL extraction). When that lands, a Codama-
// generated client replaces these stubs.

const NOT_IMPLEMENTED_AGENT_READ = {
  status: "not_implemented",
  reason: "On-chain account read requires Codama IDL client.",
  unblocker: {
    issue: 15,
    title: "Anchor 0.30 IDL extraction fails on AssociatedToken",
    workaround:
      "Read the AgentReputation PDA directly via @solana/web3.js; PDA seeds are [\"agent_reputation\", agent_pubkey] under program JDBeDr9WFhepcz4C2JeGSsMN2KLW4C1aQdNLS2jvc79G. Field layout in programs/credmesh-reputation/src/state.rs.",
  },
} as const;

const NOT_IMPLEMENTED_TX_BUILDER = {
  status: "not_implemented",
  reason: "Transaction builder requires Codama IDL client to serialize ix data.",
  unblocker: {
    issue: 15,
    title: "Anchor 0.30 IDL extraction fails on AssociatedToken",
    workaround:
      "Agent-side: call request_advance directly via @solana/web3.js with manually-borsh-encoded args. Account list and ix-arg layout in programs/credmesh-escrow/src/instructions/request_advance.rs.",
  },
} as const;

app.get("/agents/:address", (c) => {
  return c.json(
    {
      address: c.req.param("address"),
      ...NOT_IMPLEMENTED_AGENT_READ,
    },
    501,
  );
});

app.post("/agents/:address/advance", (c) => {
  return c.json(
    {
      address: c.req.param("address"),
      ...NOT_IMPLEMENTED_TX_BUILDER,
    },
    501,
  );
});

/**
 * Helius webhook ingest. Verifies the X-Helius-Auth secret (operator
 * sets HELIUS_WEBHOOK_SECRET in env), then accepts the event payload.
 *
 * Payload persistence to a SQLite derived-view cache lands together with
 * issue #42 (server route handlers); for now the endpoint validates auth
 * and acknowledges receipt without persisting. This is NOT a silent
 * placeholder — the response explicitly carries a `persistence` field
 * indicating the cache write is deferred. Operators monitoring the
 * webhook see the auth gate work; downstream consumers see the explicit
 * "not_yet_persisted" signal and can fall back to direct chain reads.
 */
app.post("/webhooks/helius", async (c) => {
  const expected = process.env.HELIUS_WEBHOOK_SECRET;
  if (expected && c.req.header("X-Helius-Auth") !== expected) {
    return c.json({ error: "unauthorized" }, 401);
  }
  let payloadCount = 0;
  try {
    const body = await c.req.json();
    payloadCount = Array.isArray(body) ? body.length : 1;
  } catch {
    payloadCount = 0;
  }
  return c.json({
    ok: true,
    received: payloadCount,
    persistence: "not_yet_persisted",
    unblocker: {
      issue: 42,
      title: "ts/server route handlers — wire SQLite derived-view cache + SSE relay",
    },
  });
});

const port = Number(process.env.PORT) || 3000;
console.log(`credmesh-solana server listening on :${port}`);
serve({ fetch: app.fetch, port });
