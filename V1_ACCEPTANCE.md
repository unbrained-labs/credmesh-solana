# V1 Acceptance criteria

What "v1 ready to ship" means. Drives sprint planning; updated as scope shifts.

Last refresh: 2026-05-03 — post-EPIC #9 + audit-driven fixes + first devnet deploy.

## On-chain

- [-] `credmesh-escrow` deployed on devnet with verified-build hash
  *(handlers + verified build complete; deploy itself pending wallet top-up — keypair reserved at `DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF`)*
  - [x] `init_pool` creates Pool + share mint + vault ATA + mints virtual-shares dead supply
  - [x] `deposit` mints shares using u128 virtual-shares math; first-depositor inflation cost ≥ 10⁶× attacker profit (Bankrun property test passing)
  - [x] `withdraw` enforces idle-only cap; fails atomically when deployed > idle
  - [x] `request_advance` (worker path) reads `Receivable` PDA via Anchor 0.30 typed `Account` + `seeds::program` (PR #30), computes credit cap from `AgentReputation`, transfers USDC, opens permanent `ConsumedPayment` PDA
  - [x] `request_advance` (ed25519 path) verifies prior ed25519 ix with offset-internal asserts (asymmetric.re/Relay fix in `crates/credmesh-shared::ix_introspection`)
  - [x] `claim_and_settle` computes 3-tranche waterfall with checked math; sum invariant holds (PR #20 property test); remainder rounds to agent; events emit
  - [x] `liquidate` marks `Advance.state = Liquidated`, decrements `Pool.deployed_amount`, applies pool-loss surcharge; `ConsumedPayment` permanence preserved (AUDIT P0-5)
  - [x] `propose_params` / `execute_params` enforce timelock; FeeCurve invariants validated at propose-time (PR #32)
  - [ ] Squads CPI verification on `propose_params` — currently a `Signer<'info>` constraint; Squads vault PDA cannot be a Signer. Needs CPI introspection helper. Tracked in `DEPLOYMENT.md § Phase 3` as a mainnet-readiness item.
- [x] `credmesh-reputation` deployed (devnet `JDBeDr9WFhepcz4C2JeGSsMN2KLW4C1aQdNLS2jvc79G`)
  - [x] `give_feedback` permissionless writes update `feedback_count` + `feedback_digest`; only `reputation_writer_authority`-signed feedback updates `score_ema` (DECISIONS Q4)
  - [x] `NewFeedback` emitted via `emit_cpi!` for 10KB log-truncation defense (PR #11)
- [x] `credmesh-receivable-oracle` deployed (devnet `ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk`)
  - [x] Worker authority writes bounded by per-tx and per-period caps
  - [x] ed25519-verified writes gated by `AllowedSigner` PDA
  - [x] Receivable PDAs namespaced by `source_kind` (PR #32) — Worker-vs-ed25519 cross-path overwrite eliminated
- [x] All cross-program reads verify owner pubkey + re-derive PDA + check 8-byte discriminator + typed deserialize (PR #30 — declarative via Anchor 0.30 `Account<T>` + `seeds::program`)

## Tests

- [-] Bankrun unit coverage for every public instruction (happy path + 2-3 error paths)
  *(11 PRs from Track D shipped; pure-math suites running; harness suites scaffolded with `expect(true).to.be.true` placeholders pending IDL fix #15)*
- [x] Property tests:
  - [x] Waterfall sum invariant (1000 random cases, PR #20)
  - [x] Share-price monotonicity (200 sequences each: deposits, withdrawals, yield, mixed; PR #20)
  - [x] First-depositor inflation defense (PR #18 + property extension in #20)
- [-] Attack fixtures (each lands alongside its fix):
  - [x] Cross-agent ed25519 replay (PR #23, scaffold)
  - [x] `ConsumedPayment` close-then-reinit (PR #24, scaffold; gates on PR #14)
  - [x] ATA substitution on `claim_and_settle` (PR #21, scaffold)
  - [x] Sysvar instructions spoofing (PR #22, scaffold)
- [ ] Devnet end-to-end: full advance lifecycle (deposit → request → settle → withdraw) with real Circle USDC
  *(blocked on escrow deploy + IDL extraction fix)*

## Off-chain

- [-] Hono server with SIWS auth middleware (auth.ts works; route handlers stubbed)
- [ ] `buildRequestAdvanceTx` returns ready-to-sign base64 `VersionedTransaction` with PayAI as fee payer
- [ ] Helius webhook ingest with `X-Helius-Auth` check; events update SQLite derived-view cache
- [x] Three-key topology enforced on-chain (fee-payer / oracle-worker / reputation-writer separated; caps in `OracleConfig`)

## Dashboard

- [ ] React 19 + Vite + Tailwind v4 served at `/app` from server
- [ ] Phantom Connect SDK + ConnectorKit multi-wallet support
- [ ] Read-side Helius API calls all server-proxied (no `NEXT_PUBLIC_HELIUS_API_KEY`)
- [ ] Live timeline via SSE-relayed `accountSubscribe`

*(`ts/dashboard/` is currently empty. Real product gap; see `AUDIT.md § Post-EPIC` and the gaps research report.)*

## Audit + governance

- [x] Internal multi-pass audit on `credmesh-escrow` + `credmesh-reputation` + `credmesh-receivable-oracle` (4 Claude code-reviewers + Kimi K2 independent-model audit; 3 real MED findings fixed in PR #32, 1 compile-discovered fix in PR #34)
- [ ] **External** independent audit firm engagement
- [ ] Squads v4 multisig deployed for protocol governance with timelock
- [ ] All program upgrade authorities transferred to Squads vault (currently `6kWsEUqzLNaJgKbkstJUtYFWq56E1ZyYDeQ25XjChm7X`, the deployer wallet)
- [x] Verified-build commit hashes published — see `DEPLOYMENT.md § Devnet deploy log`. Both deployed binaries SHA256-match local builds byte-for-byte.

## Documentation

- [x] DECISIONS.md — design questions resolved
- [x] AUDIT.md — fixes applied + post-EPIC postscript
- [x] DESIGN.md — implementer spec
- [x] DEPLOYMENT.md — Docker recipe + deploy log + key rotation procedure
- [x] CONTRIBUTING.md
- [x] docs/ARCHITECTURE.md — program structure + PDAs (Mermaid)
- [x] docs/LOGIC_FLOW.md — 9 sequence diagrams + invariants table
- [ ] Public docs site (or comprehensive README §s for): agent onboarding, LP onboarding, governance procedures, migration to mainnet
- [ ] Threat model write-up (DESIGN §10 expanded)

## Mainnet readiness gates

Each must be green before mainnet flip:

1. [ ] ≥ 7 days of devnet operation with synthetic load (≥ 100 advances issued + settled)
2. [-] Audit findings all resolved or accepted with documented rationale *(internal — pending external)*
3. [ ] Squads governance multisig configured (members, threshold, timelock)
4. [ ] Three-key topology rotated at least once on devnet (proves the rotation flow works)
5. [ ] Hard caps active: `max_advance_pct_bps = 3000`, `max_advance_abs = 100_000_000` (= $100)
6. [ ] Insurance buffer: protocol treasury seeded with at least 5% of expected vault TVL
7. [ ] IDL extraction fix (issue #15) so TS clients can construct typed instructions

## v1 explicitly NOT in scope (deferred)

Per DESIGN §9:
- ML-derived credit curves
- Mobile Wallet Adapter / Solana Mobile
- Hyperliquid Lazer publisher
- Light Protocol compressed PDAs
- Plain-EOA agents (Squads-only for v1)
- Multi-asset pools (USDC only)
- Per-instruction-type timelock granularity
- Token-2022 USDC handling (DRAFT spike in PR #31, never merge to main per starter prompt)
- Embedded-wallet (Phantom Portal) auth
- ~~Permissionless `claim_and_settle` cranking~~ **landed via DECISIONS Q9** — SPL `Approve` delegate granted in `request_advance`; two-mode dispatch in handler. See `research/CONTRARIAN-permissionless-settle.md`.
- Multi-issuer SAS attestations (deferred to v1.5; schema documented now)

## Legend

- `[x]` complete
- `[-]` partial / in flight
- `[ ]` not started
