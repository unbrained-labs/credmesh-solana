/**
 * CredMesh Solana — backend server.
 *
 * Hono on Node. Three live endpoints:
 *   GET /                        — service identity
 *   GET /health                  — health probe
 *   GET /.well-known/agent.json  — A2A agent card consumed by the
 *                                  cross-lane outreach agent
 *   POST /auth/nonce             — SIWS nonce mint
 *
 * On-chain reads (agent reputation, request_advance tx-builder) live on
 * EVM and the bridge. Solana programs are called directly by clients
 * with @solana/kit; no server-side wrappers needed in this service.
 */

import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { issueNonce } from "./auth.js";

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
    version: "0.1.0",
    docs: "https://github.com/unbrained-labs/credmesh-solana",
  }),
);

app.get("/health", (c) => c.json({ ok: true }));

/**
 * A2A agent card. Built once at module load (env is fixed at startup).
 * Consumed by the cross-lane outreach agent at
 * `../trustvault-credit/packages/outreach-agent`.
 *
 * Required env (omitting OUTREACH_VAULT_ADDRESS or OUTREACH_CHAIN_ID
 * drops the `outreach` block; outreach agent then treats us as not-ready):
 *   OUTREACH_CHAIN_ID            "solana-devnet" or "solana-mainnet"
 *   OUTREACH_VAULT_ADDRESS       Pool PDA
 *   OUTREACH_EXPLORER_BASE       default https://solscan.io
 *   OUTREACH_MCP_PACKAGE         default @credmesh/mcp-solana
 *   OUTREACH_SOURCE_REPO         default the GitHub URL
 *   PUBLIC_API_BASE              default https://credmesh.xyz
 */
const AGENT_CARD = buildAgentCard();
app.get("/.well-known/agent.json", (c) => c.json(AGENT_CARD));

function buildAgentCard(): Record<string, unknown> {
  const apiBase = process.env.PUBLIC_API_BASE ?? "https://credmesh.xyz";
  const card: Record<string, unknown> = {
    name: "CredMesh Solana",
    description:
      "Unsecured credit for autonomous agents on Solana. EVM-attested reputation; agents borrow against their standing credit line and self-settle when paid.",
    a2a: { endpoint: `${apiBase}/.well-known/agent.json`, version: "0.1" },
    capabilities: ["lp-deposit", "credit-advance", "agent-self-settle"],
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
        "CredMesh-Solana lends short-duration credit to autonomous agents",
        "against their EVM-attested reputation. LPs deposit USDC into the",
        "Pool PDA. Agents draw against their standing credit line — no",
        "collateral, no escrow. They settle from their own funds when",
        "paid. Default → liquidation → LP loss → reputation crash on EVM.",
        "Protocol takes 15% of fees; 85% to LPs. Three-key topology and",
        "Squads-governed FeeCurve updates with timelock.",
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

const port = Number(process.env.PORT) || 3000;
console.log(`credmesh-solana server listening on :${port}`);
serve({ fetch: app.fetch, port });
