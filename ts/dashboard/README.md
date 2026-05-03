# CredMesh Dashboard

LP / Agent / Governance views for the CredMesh-Solana protocol.

Stack: React 19 + Vite 5 + Tailwind v4 + Phantom (wallet-adapter) + TanStack Query + `@solana/web3.js`.

## Status

**Hackathon scaffold.** All on-chain reads/writes are mocked behind stub handlers
that log the would-be-tx and `alert()` so the UI is fully navigable before the
Codama IDL client lands. Replace mocks once the Worker IDL track ships:

- `src/lib/mock-data.ts` — pool / advance / share-price seed data
- `src/views/*.tsx` — every form's `onSubmit` has a `// TODO: real client (issue #15)` marker

## Run

```bash
npm install
npm run dev          # http://localhost:5173
npm run build        # tsc -b && vite build
npm run typecheck    # tsc --noEmit
```

Set `VITE_SOLANA_RPC` to override the default devnet endpoint (e.g. a Helius
key proxied via `ts/server`).

## Layout

```
src/
├── App.tsx                           Router shell
├── main.tsx                          Buffer polyfill + StrictMode
├── index.css                         Tailwind v4 + wallet-adapter overrides
├── providers/SolanaProviders.tsx     Connection + Wallet + QueryClient
├── components/                       Layout, Card, Stat, SharePriceChart
├── lib/                              format helpers + mock data
└── views/
    ├── LpView.tsx                    Pool stats, deposit, withdraw, chart
    ├── AgentView.tsx                 Outstanding advances, request advance
    └── GovernanceView.tsx            Fee curve, propose/execute, skim fees
```

## Conventions

- All financial figures use `tnum` (tabular nums) and `font-mono` for stable column alignment.
- USDC values are stored as raw `bigint` micro-USDC; the `formatUsdc()` helper
  is the only place that converts to display strings.
- Public keys + receivable IDs are abbreviated via `shortAddr()` for visual scan.
- The `// TODO: real client (issue #15)` comment is the search anchor for
  every callsite that needs to swap mock data for a Codama-generated client.
