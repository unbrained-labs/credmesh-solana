# V1 Acceptance criteria

What "v1 ready to ship" means. Drives sprint planning; updated as scope shifts.

## On-chain

- [ ] `credmesh-escrow` deployed on devnet with verified-build hash
  - [ ] `init_pool` creates Pool + share mint + vault ATA + mints virtual-shares dead supply
  - [ ] `deposit` mints shares using u128 virtual-shares math; first-depositor inflation cost ≥ 10⁶× attacker profit (Bankrun property test)
  - [ ] `withdraw` enforces idle-only cap; fails atomically when deployed > idle
  - [ ] `request_advance` (worker path) correctly reads `Receivable` PDA, computes credit cap from `AgentReputation`, transfers USDC, opens permanent `ConsumedPayment` PDA
  - [ ] `request_advance` (ed25519 path) verifies prior ed25519 ix with offset-internal asserts (asymmetric.re/Relay fix)
  - [ ] `claim_and_settle` computes 3-tranche waterfall with checked math; sum invariant holds; remainder rounds to agent; events emit
  - [ ] `liquidate` marks `Advance.state = Liquidated`, decrements `Pool.deployed_amount`, applies pool-loss surcharge
  - [ ] `propose_params` / `execute_params` enforce timelock + Squads CPI verification
- [ ] `credmesh-reputation` deployed
  - [ ] `give_feedback` permissionless writes update `feedback_count` + `feedback_digest`; only `reputation_writer_authority`-signed feedback updates `score_ema` (DECISIONS Q4)
- [ ] `credmesh-receivable-oracle` deployed
  - [ ] Worker authority writes bounded by per-tx and per-period caps
  - [ ] ed25519-verified writes gated by `AllowedSigner` PDA
- [ ] All cross-program reads verify owner pubkey + re-derive PDA + check 8-byte discriminator + typed deserialize

## Tests

- [ ] Bankrun unit coverage for every public instruction (happy path + 2-3 error paths)
- [ ] Property tests:
  - [ ] Waterfall sum invariant
  - [ ] Share-price monotonicity (no fee accrual reduces share price)
  - [ ] First-depositor inflation defense
- [ ] Attack fixtures (each lands alongside its fix):
  - [ ] Cross-agent ed25519 replay → fails
  - [ ] `ConsumedPayment` close-then-reinit → fails (close path doesn't exist)
  - [ ] ATA substitution on `claim_and_settle` → fails
  - [ ] Sysvar instructions spoofing → fails
- [ ] Devnet end-to-end: full advance lifecycle (deposit → request → settle → withdraw) with real Circle USDC

## Off-chain

- [ ] Hono server with SIWS auth middleware, verified by Phantom + Solflare + Backpack injected wallets
- [ ] `buildRequestAdvanceTx` returns ready-to-sign base64 `VersionedTransaction` with PayAI as fee payer
- [ ] Helius webhook ingest with `X-Helius-Auth` check; events update SQLite derived-view cache
- [ ] Three-key topology enforced (fee-payer, oracle-worker, reputation-writer separate)

## Dashboard

- [ ] React 19 + Vite + Tailwind v4 served at `/app` from server
- [ ] Phantom Connect SDK + ConnectorKit multi-wallet support
- [ ] Read-side Helius API calls all server-proxied (no `NEXT_PUBLIC_HELIUS_API_KEY`)
- [ ] Live timeline via SSE-relayed `accountSubscribe`

## Audit + governance

- [ ] One independent audit pass on `credmesh-escrow` + `credmesh-reputation`
- [ ] Squads v4 multisig deployed for protocol governance with timelock
- [ ] All program upgrade authorities transferred to Squads vault
- [ ] Verified-build commit hashes published in repo

## Documentation

- [x] DECISIONS.md — design questions resolved
- [x] AUDIT.md — fixes applied
- [x] DESIGN.md — implementer spec
- [x] CONTRIBUTING.md
- [ ] Public docs site (or comprehensive README §s for): agent onboarding, LP onboarding, governance procedures, migration to mainnet
- [ ] Threat model write-up (DESIGN §10 expanded)

## Mainnet readiness gates

Each must be green before mainnet flip:

1. ≥ 7 days of devnet operation with synthetic load (≥ 100 advances issued + settled)
2. Audit findings all resolved or accepted with documented rationale
3. Squads governance multisig configured (members, threshold, timelock)
4. Three-key topology rotated at least once on devnet (proves the rotation flow works)
5. Hard caps active: `max_advance_pct_bps = 3000`, `max_advance_abs = 100_000_000` (= )
6. Insurance buffer: protocol treasury seeded with at least 5% of expected vault TVL

## v1 explicitly NOT in scope (deferred)

Per DESIGN §9:
- ML-derived credit curves
- Mobile Wallet Adapter / Solana Mobile
- Hyperliquid Lazer publisher
- Light Protocol compressed PDAs
- Plain-EOA agents (Squads-only for v1)
- Multi-asset pools (USDC only)
- Per-instruction-type timelock granularity
- Token-2022 USDC handling
- Embedded-wallet (Phantom Portal) auth
- Permissionless `claim_and_settle` cranking
- Multi-issuer SAS attestations (deferred to v1.5; schema documented now)
