# TypeScript packages

Mirrors the EVM repo structure. Pre-implementation.

```
ts/
├── server/        Hono backend; SIWS auth, tx-builder, Helius webhooks.
│                  Replaces packages/credit-worker from the EVM repo.
├── dashboard/     React 19 + Vite + Tailwind v4 + Phantom Connect SDK + ConnectorKit.
│                  Forks packages/dashboard, swapping wagmi/viem → Solana.
└── mcp-server/    HTTP-API wrapper for the public CredMesh API. No chain code.
                   Direct port of packages/mcp-server.
```

See `DESIGN.md` §6 for the auth, tx-builder, and webhook integration spec.

The escrow Codama TS client is generated from the Anchor IDL and consumed by
`ts/server` and `ts/dashboard`. Run `anchor build && codama run` (script TBD).
