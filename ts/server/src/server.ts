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

app.get("/agents/:address", (c) => {
  return c.json({
    address: c.req.param("address"),
    note: "TODO: read agent state from on-chain PDAs + derived-view cache",
  });
});

app.post("/agents/:address/advance", (c) => {
  return c.json({
    status: "TODO",
    message: "buildRequestAdvanceTx — returns base64 unsigned VersionedTransaction. See DESIGN §6.",
  });
});

app.post("/webhooks/helius", async (c) => {
  const expected = process.env.HELIUS_WEBHOOK_SECRET;
  if (expected && c.req.header("X-Helius-Auth") !== expected) {
    return c.json({ error: "unauthorized" }, 401);
  }
  return c.json({ ok: true, ingested: 0 });
});

const port = Number(process.env.PORT) || 3000;
console.log(`credmesh-solana server listening on :${port}`);
serve({ fetch: app.fetch, port });
