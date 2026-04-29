# ts/server

Hono backend for CredMesh Solana. Skeleton in place; handler bodies pending.

## Run

```bash
cd ts/server
npm install
npm run dev      # tsx watch on src/server.ts → http://localhost:3000
npm run typecheck
```

## What's here

- `src/server.ts` — Hono app, CORS allowlist (matches EVM repo), nonce + auth routes, write-mount auth middleware, webhook ingest.
- `src/auth.ts` — SIWS auth middleware (Sign-In With Solana, CAIP-122). Verifies ed25519 detached signatures via `tweetnacl`.
- `src/pricing.ts` — direct port of `packages/credit-worker/src/pricing.ts` from the EVM repo. Identical 4-component fee math; the same parameters live on-chain in `Pool.fee_curve` so the on-chain program enforces them.

## What's missing (pending Anchor handler implementation)

- `buildRequestAdvanceTx` — server endpoint that constructs an unsigned `VersionedTransaction` (with Kora/PayAI fee-payer pre-set, blockhash, ALT, ed25519 verify ix if `source_kind != Worker`) and returns it as base64. Agent signs and submits.
- Codama-generated escrow/reputation/oracle TS clients. Generate with `anchor build && codama run` once handlers are implemented.
- Helius SDK wiring for: `getPriorityFeeEstimate`, webhook lifecycle management, DAS asset reads (for MPL Agent Registry assets).
- SQLite derived-view cache for the dashboard timeline.

## Env vars

```
PORT=3000
CREDMESH_DOMAIN=credmesh.xyz                # for SIWS message
HELIUS_API_KEY=...                          # server-side only — never expose
HELIUS_WEBHOOK_SECRET=...                   # X-Helius-Auth shared secret
PAYAI_FACILITATOR_URL=https://facilitator.payai.network
SOLANA_MAINNET_RPC_URL=https://...
SOLANA_DEVNET_RPC_URL=https://...
SOLANA_MAINNET_USDC_MINT=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
SOLANA_DEVNET_USDC_MINT=4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU
SOLANA_MAINNET_ESCROW_PROGRAM=...           # set after first deploy
SOLANA_MAINNET_REPUTATION_PROGRAM=...
SOLANA_MAINNET_RECEIVABLE_ORACLE_PROGRAM=...
ORACLE_WORKER_KEYPAIR=...                   # base58; separate from fee-payer
REPUTATION_WRITER_KEYPAIR=...               # base58; separate from oracle worker
```

Three-key topology per DESIGN §10: fee-payer (PayAI hosted), oracle worker authority, reputation writer authority. Never collapse them.
