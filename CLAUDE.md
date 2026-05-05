# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

CredMesh-Solana is a pre-implementation port of [CredMesh](https://github.com/unbrained-labs/credmesh) (programmable credit protocol for autonomous agents) from EVM (Base) to Solana. The EVM protocol is live at https://credmesh.xyz; this repo is the Anchor scaffold + design + research that gets it to Solana.

Status: handler bodies written end-to-end but **not compile-verified**. First `anchor build` will surface 1-3 minor Anchor 0.30 syntax fixes.

## Read order (before touching code)

1. **`DECISIONS.md`** — five blocking design questions answered with rationale (MPL Agent Registry vs SATI, Squads onboarding flow, Sybil mitigation, SAS roadmap, fee-payer infra).
2. **`AUDIT.md`** — three independent reviews + final-review pass; all P0/P1 fixes applied with cross-references in code as `// AUDIT P0-X` comments.
3. **`DESIGN.md`** — implementer spec (programs, PDAs, instructions, invariants, threat model).
4. **`research/HANDLER_PATTERNS.md`** — canonical Solana lending patterns (MarginFi/Solend/Kamino/Drift/Squads) lifted byte-for-byte at pinned commit hashes. **The handler-implementation reference manual.**
5. **`V1_ACCEPTANCE.md`** — what "v1 ready to ship" means.
6. **`DEPLOYMENT.md`** — devnet/mainnet deploy procedure + key rotation.
7. **`research/CONTRARIAN.md`** — why we redesigned where we did (vs literal EVM port).
8. **`research/REVIEW.md`**, `research/SYNTHESIS.md`, `research/01-04` — supporting research.

## Commands

```bash
# Toolchain (see CONTRIBUTING.md for full setup)
rustup default 1.79.0
sh -c "$(curl -sSfL https://release.anza.xyz/v1.18.26/install)"
cargo install --git https://github.com/coral-xyz/anchor avm --force && avm install 0.30.1

anchor build                                 # build all 4 programs
anchor test --skip-local-validator           # bankrun tests
npm install && npm test                      # ts-mocha + bankrun

# TS server
cd ts/server && npm install && npm run dev   # http://localhost:3000
```

## Architecture

### Workspace layout

```
crates/
└── credmesh-shared/                 seeds, program IDs, helpers (mpl_identity,
                                     cross_program, ix_introspection, ed25519_message).
                                     Library only — never deployed.
programs/
├── credmesh-escrow/                 Pool vault + share-mint + advance + waterfall
├── credmesh-reputation/             8004-shape rolling-digest reputation
└── credmesh-receivable-oracle/      worker + ed25519 payer-signed receivables
ts/server/                           Hono backend (SIWS auth, tx-builder, webhook ingress)
scripts/                             deploy.ts + init_oracle.ts + init_pool.ts
tests/bankrun/                       anchor-bankrun unit/integration + AUDIT-finding fixtures
docs/                                ARCHITECTURE.md + LOGIC_FLOW.md (Mermaid diagrams)
```

NB: `credmesh-shared` lives in `crates/`, not `programs/`. Anchor traverses every
`programs/*` looking for `#[program]` modules; a library would error there.

### External programs CredMesh **uses** but does not deploy

- **Squads v4** (`SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf`) — agent vaults, SpendingLimits, governance multisig
- **MPL Agent Registry** (`1DREGFgysWYxLnRnKQnwrxnJQeSMk2HmGaC6whw2B2p`) + **MPL Agent Tools** (`TLREGni9ZEyGC3vnPZtqUh95xQ8oPqJSvNjvB7FGK8S`) — agent identity + DelegateExecutionV1
- **MPL Core** (`CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d`) — base asset primitive
- **SPL Token classic** — USDC vault and share mint
- **ed25519 program** + **Memo v2** — receivable verification + replay-nonce binding

## Conventions specific to this repo

- **Cross-program reads use the four-step verify** (owner → address → discriminator → typed deserialize) via `credmesh_shared::cross_program::read_cross_program_account<T>`. Forgetting any step is the Wormhole-class bug. **Don't skip steps.**
- **PDA seeds come from `credmesh-shared::seeds`.** Never re-declare seed bytes in two crates — they will silently drift.
- **`ConsumedPayment` is permanent** (AUDIT P0-5). Closing it reopens close-then-reinit replay. Don't add a close handler.
- **`request_advance` has no pause path.** Advance issuance is never gated by governance. The "no pause on issuance" invariant is load-bearing — see DESIGN §3.5 and AUDIT P0-6.
- **`claim_and_settle` is three-mode** (DECISIONS Q9 + Q10). Mode A = agent self-cranks; Mode B = relayer settles via SPL delegate granted at `request_advance`; Mode 3 = cranker funds repayment from own ATA (EVM-parity `settle(advanceId, payout)` — marketplace pays directly with its own USDC, agent never involved). Source-of-funds owner pinned dynamically to {agent, cranker}; substitution defenses don't depend on cranker identity. Full design: `research/CONTRARIAN-permissionless-settle.md`.
- **EVM-parity credit-line model is the v1 default** (BRUTAL-TRUTH-EVM-PARITY-DRIFT.md, DECISIONS Q3 amended). AgentReputation has `credit_limit_atoms` + `outstanding_balance_atoms`; underwriting enforces `available = limit - outstanding` (EVM `availableCredit`). Score formula in `programs/credmesh-reputation/src/scoring.rs` is a port of `credit-worker/src/credit.ts:24-56`. **MPL Core and Squads-as-configAuthority are OPT-IN.** Raw-keypair agents are first-class; `register_agent` is one tx with the agent as signer.
- **Three-key topology** (DESIGN §10): fee-payer, oracle worker authority, reputation writer authority must NEVER share keys. The off-chain config and rotation flow enforce this.
- **All math is `checked_*`** or wrapped in u128. Cargo.toml sets `overflow-checks = true` in release; don't rely on it.
- **Errors map to typed enums** (`CredmeshError`, `ReputationError`, `OracleError`). No `unwrap()` in handlers.
- **Cross-program seed constants come from `credmesh-shared`.** Each program `pub use`s only the seeds it actually uses, so its own clients can derive PDAs without depending on `credmesh-shared`.
- **Events are emitted as the LAST step of each handler.** A partial failure mid-handler shouldn't emit a misleading event.
- **No `find_program_address` in hot paths** when the bump is already cached. Per HANDLER_PATTERNS.md Pattern 2 (~1500 CU per call). Use `Pubkey::create_program_address(seeds_with_bump, program_id)` instead.
- **Use `transfer_checked` not bare `transfer`.** All 6 transfer call sites in credmesh-escrow have been migrated. anchor-spl 0.30.1 does NOT expose `mint_to_checked` or `burn_checked` — those stay as bare `mint_to` / `burn` (one site each: `deposit::handler` and `withdraw::handler`); upgrade when anchor-spl ships the wrappers.
- **`emit!` is the LAST line.** Anti-pattern: emitting before all CPIs succeed.

## What NOT to do

- Don't add a `paused` field back to `Pool`. The "no pause on issuance" invariant is load-bearing — AUDIT P0-6.
- Don't close `ConsumedPayment`. Permanent. AUDIT P0-5.
- ~~Don't make `claim_and_settle` permissionless in v1. AUDIT P0-3/P0-4.~~ **Superseded by DECISIONS Q9 — permissionless cranking landed via SPL `Approve` delegate. Two-mode dispatch in handler. See `research/CONTRARIAN-permissionless-settle.md`.**
- Don't introduce Light Protocol compressed PDAs or Token-2022 features in v1. Both are explicitly v2+.
- Don't add per-record SQL persistence on the off-chain server. State migrates to on-chain PDAs (DESIGN §6); SQLite is a derived-view cache only.
- Don't use `init_if_needed` for replay-protection PDAs — only `init`. AUDIT P0-5.
- ~~Don't use bare `transfer` — Token-2022 forward-compat requires `transfer_checked`.~~ **Done — 0 bare transfer sites in credmesh-escrow as of 2026-05-06. Bare `mint_to` and `burn` remain at 1 site each because anchor-spl 0.30.1 doesn't expose checked variants for those.**

## V1 explicitly NOT in scope (deferred)

Per DESIGN §9:
- ML-derived credit curves
- Mobile Wallet Adapter / Solana Mobile
- Hyperliquid Lazer publisher
- Light Protocol compressed PDAs
- ~~Plain-EOA agents (Squads-only for v1)~~ **REVERSED 2026-05-06 — raw-keypair agents are first-class; Squads is opt-in. DECISIONS Q3 amended.**
- Multi-asset pools (USDC only)
- Per-instruction-type timelock granularity
- Token-2022 USDC handling (Circle hasn't migrated)
- Embedded-wallet (Phantom Portal) auth
- ~~Permissionless `claim_and_settle` cranking~~ **landed in v1 — DECISIONS Q9**
- Multi-issuer SAS attestations (deferred to v1.5; schema documented now)

## Sister repo

The original EVM CredMesh lives at `../credmesh/`. When porting logic (e.g., `pricing.ts` → `compute_fee_amount` in escrow), the EVM file is the source of truth for math; the Solana file mirrors it. Tests assert the on-chain quote and the off-chain quote match exactly.
