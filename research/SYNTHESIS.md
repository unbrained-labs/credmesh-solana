# SYNTHESIS — CredMesh on Solana

End-to-end architecture proposal, reconciled from the four parallel research reports. Read this first; the four numbered docs are the supporting detail.

## TL;DR

Build CredMesh on Solana as **three on-chain programs + one off-chain worker + a forked dashboard**, and reuse the Solana ecosystem wherever possible:

- **Identity:** don't build — **integrate the Solana Agent Registry** (ERC-8004 port at `solana.com/agent-registry`, reference impl `QuantuLabs/8004-solana`).
- **Reputation:** ride on the same `8004-solana` reputation module (rolling-digest PDA + program events).
- **Escrow + LP vault:** **custom Anchor program** (`credmesh_escrow`) with cToken-style share mint, idle-only withdrawal accounting, per-advance PDA, and pluggable oracle CPIs.
- **Oracles:** small Anchor programs (`registry_receivable`, `reputation_credit`, `hyperliquid_receivable`) the worker writes to and the escrow reads via `account_info` — no CPI tax for reads.
- **Worker:** Hono on Node-tsx unchanged. Swap viem → `@solana/kit` + Codama-generated clients + Helius SDK. Replace EIP-191 with **SIWS**. Use **Helius webhooks** for events and **Helius Sender** for tx submission.
- **Dashboard:** swap wagmi/viem → **Phantom Connect SDK** (primary) + **Solana ConnectorKit** (multi-wallet). Same React 19 + Vite + Tailwind v4.
- **Payments:** **x402 is now native on Solana** (Kora facilitator). Drop the EIP-3009 path; partial-sign + relayer is the new shape.
- **Cross-chain:** **CCTP v2** for Solana ↔ Base bridging; build it as a fallback so an agent with credit on one chain can pay on the other.

## EVM → Solana mapping table

| EVM piece (today) | Solana counterpart | Build vs adopt |
|---|---|---|
| `IdentityRegistry.sol` (ERC-8004) | Solana Agent Registry / `8004-solana` Identity module — Metaplex Core NFT + `["agent_metadata", asset]` PDA | **Adopt** |
| `ReputationRegistry.sol` (append-only score history) | `8004-solana` Reputation module — rolling keccak digest PDA + program events; ATOM Engine for EMA | **Adopt** |
| `TrustlessEscrowV3.sol` (ERC-4626 + escrow) | Custom `credmesh_escrow` Anchor program; one Pool PDA per asset, SPL share-mint, per-agent Advance PDA, idle-only withdrawal via `deployed_amount` accounting | **Build** |
| Pluggable oracles (`ReputationCreditOracle`, `RegistryReceivableOracle`, `HyperliquidReceivableOracle`, `ReputationOnlyOracle`) | Separate Anchor programs each owning a per-agent PDA. Escrow reads via `remaining_accounts` (no CPI), oracle writes via worker or permissionless callers | **Build (small)** |
| `engine.ts` waterfall (`treasury.ts`) | Single `claim_and_settle` instruction: protocol → LP vault → agent net via three CPI'd `TransferChecked` calls in one ix. Reference: CascadePay | **Build** |
| `consumedPayments` (replay) | `consumedSignatures` keyed by 64-byte tx signature (or `keccak(sig ‖ receivable_id)` for x402 ergonomics) | **Port** |
| `chains.ts` (env-var registry) | Same shape, prefix `SOLANA_MAINNET_*` / `SOLANA_DEVNET_*`. `chains.ts` gains a `kind: "evm" \| "svm"` discriminant during transition | **Port** |
| `chain.ts` (viem ops) | New `solana.ts`: `@solana/kit` + Codama escrow client + Helius RPC + Helius Sender | **Rewrite** |
| EIP-191 auth middleware | SIWS (Sign-In With Solana) — `tweetnacl.sign.detached.verify`. Headers: `X-Agent-Address` (base58 pubkey), `X-Agent-Signature` (base58), `X-Agent-Timestamp`, `X-Agent-Cluster`, `X-Agent-Nonce` | **Rewrite** |
| `buildAdvanceCalldata()` (trustless mode) | `buildAdvanceTransaction()`: returns base64 unsigned `VersionedTransaction` (worker computes blockhash, fee payer, ALTs, CU budget). Agent signs and submits | **Rewrite** |
| Coinbase x402 / EIP-3009 | x402 native on Solana — partial-sign + relayer (Kora). USDC SPL `TransferChecked` instead of `transferWithAuthorization` | **Rewrite, same shape** |
| Tempo / Stripe MPP | **Coinbase Onramp** (0% USDC) primary, **Stripe Crypto Onramp** secondary. Off-ramp via Coinbase Offramp | **Rewire** |
| `mintTestTokens` (TestUSDC.sol) | Self-deployed devnet SPL mint **+** Circle devnet faucet (`faucet.circle.com`) | **Both** |
| Wallet-watching / event ingestion | Helius webhooks (engine source-of-truth) + Helius Enhanced WebSockets relayed via SSE (dashboard live UI) | **Rewrite** |
| Dashboard wagmi/viem | Phantom Connect SDK + Solana ConnectorKit; backend proxies all Helius calls except Sender | **Rewrite** |
| MCP server | No change (HTTP API only) | **No change** |

## Recommended program layout

```
credmesh-solana/
├── programs/
│   ├── credmesh-escrow/          NEW — vault, advance, settlement, governance
│   │   └── PDAs:
│   │       ├── ["pool", asset_mint]
│   │       ├── ["advance", agent, receivable_id]
│   │       └── ["protocol_treasury"]
│   ├── registry-receivable/      NEW — worker-written per-agent receivable
│   ├── reputation-credit/        NEW — derives max-credit from reputation events
│   ├── hyperliquid-receivable/   NEW — worker-attested HL position oracle
│   └── (8004-solana programs)    ADOPTED — identity + reputation
└── (escrow CPIs into oracle programs OR reads their PDAs via remaining_accounts)
```

Three guarantees the design preserves from EVM:
1. **No admin can approve advances.** `request_advance` has no governance signer.
2. **No pause.** Don't write one. (USDC mint pause by Circle is the only stop button on either chain.)
3. **Timelocked governance.** Squads v4 multisig holds program upgrade authority + `Pool.governance`. All param changes go through `propose_params → wait → execute_params`.

## Repayment instruction (the core money path)

Single Anchor instruction `claim_and_settle` invoked when an agent's job pays into the per-job escrow PDA. In one instruction:
1. Verify caller (job authority post-payment, or permissionless after a time-lock).
2. Read amounts from job PDA: `principal_owed`, `accrued_fees`, `late_penalty`.
3. CPI three `TransferChecked` calls in order: **protocol cut (15%) → LP vault (85% + principal repayment) → agent net**.
4. Update `Pool.deployed_amount -= principal`, `total_assets += fee_share` (LPs realize yield via share-price increase, not new mints).
5. Close job PDA on full settlement.

This is the Solana equivalent of `treasury.ts:settleWaterfall`. Read CascadePay's contract before writing it; don't fork it (their 1% fee is baked in).

## Off-chain port shopping list

- **`@solana/kit`** + **Codama**-generated client off the escrow IDL. Skip Anchor TS client; it's still web3.js v1.
- **Helius SDK** (`helius-sdk`) for `getPriorityFeeEstimate`, `parseTransactions`, webhook management, `getWalletHistory` / `getWalletIdentity` / `getWalletFundedBy` for credit scoring.
- **Helius Sender** (`https://sender.helius-rpc.com/fast`) for all writes. 0.0002 SOL Jito tip, `skipPreflight: true`, app-side retry.
- **`tweetnacl`** for SIWS verification.
- **`@solana-program/{system,token,compute-budget,memo}`** for canonical instructions.
- **Address Lookup Tables (ALTs)** for the escrow program's request_advance ix (must reference oracle programs + their data accounts in one tx).

## Credit-scoring upgrade (don't just port — improve)

Solana wallet intelligence is **richer** than EVM equivalents. Add three signal tiers to the credit pipeline:

- **Tier 1 (gating):** Helius `getWalletIdentity` (sanctions / mixer detection), `getWalletFundedBy` (provenance, depth-3 recursive), DFlow Proof KYC (`/verify/{address}`). Failures cap or reject — never solely score.
- **Tier 2 (cashflow):** Helius `getWalletHistory` over 90 days — sustained USDC/USDT/PYUSD inflows are credit ceiling proxy. EVM had no equivalent.
- **Tier 3 (smart-money bonus):** OKX smart-money flags + DFlow agent CLI history. Bonus, never sole signal.

Cache scores 24h in worker state; recompute on advance request.

## Phased delivery plan

1. **Devnet escrow MVP** — `credmesh_escrow` program, plain `RegistryReceivableOracle`, single Pool, hard caps ($25 unverified / $250 KYC'd). Worker calls Helius RPC + Sender. CI fixtures via self-deployed test mint.
2. **Identity + reputation integration** — adopt `8004-solana`, mint Agent Registry NFT per CredMesh agent at registration, write reputation events on settle. SAS attestations for "post-settlement feedback" optional.
3. **Trustless mode** — `buildAdvanceTransaction` server endpoint returns base64 unsigned tx; agent signs and submits via Sender.
4. **Mainnet-beta staging** — keep $10–$100 caps, real Circle USDC, real Helius webhooks. Skip public testnet (Solana mainnet costs ~$0.0001/tx).
5. **x402 native** — Kora facilitator integration; agents pay tools natively on Solana.
6. **CCTP fallback** — Wormhole CCTPExecutorRoute for cross-chain; agent with credit on Solana can pay an EVM x402 server.
7. **Cross-chain identity bridging** — agent's `registrations` JSON references both EVM `IdentityRegistry` entry and Solana Agent Registry asset; `crossChainIdentities[]` in the agent card with wallet-binding signatures.

## Top open questions to resolve before coding

1. **Single program or split?** — Recommended split: `credmesh_escrow` + 3 oracles is cleaner than a monolith and matches the EVM layout.
2. **Agent NFT vs pure PDA identity** — recommend Metaplex Core NFT (Agent Registry standard); accept higher cost (~0.009 SOL vs 0.002) for ecosystem fit.
3. **Per-advance NFT?** Jupiter Lend wraps borrow positions as NFTs. CredMesh advances aren't transferable — start with plain PDAs; revisit if explorer/portfolio UX demands it.
4. **First-depositor inflation defense** — confirm: program mints "dead" shares to itself at pool init.
5. **Replay key shape** — `consumedSignatures[sig]` vs `consumedPayments[keccak(sig ‖ receivable_id)]`. Prefer the latter for x402 ergonomics.
6. **Phantom Portal App ID** — who owns this org account? Required for embedded-wallet OAuth on the dashboard.
7. **State scaling** — single-process SQLite blob assumes one worker. Webhook ingest at 10s/sec may pressure that. Measure before scaling out.
8. **Hyperliquid Lazer feed** — currently unavailable; defaults to worker-attested HL receivable.

## What stays the same

- Hono + tsx + better-sqlite3 + single-process state assumption
- Single `kv` row state model — don't add per-record SQL
- 15% protocol / 85% LP fee split, 4-component dynamic pricing, 2–25% fee range
- Hard caps: 30% of receivable, $100 absolute (demo stage)
- Replay-protected payment proofs
- `assertAuthorized(verifiedAddress, target)` semantics in handlers
- MCP server (chain-agnostic)
- Dashboard React 19 + Vite + Tailwind v4 stack (only the wallet/RPC layer changes)
