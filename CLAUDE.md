# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

CredMesh-Solana is the Solana credit-issuance + settlement venue for the
[CredMesh](https://github.com/unbrained-labs/credmesh) protocol. The EVM lane
(live at https://credmesh.xyz) is the **single source of truth for identity,
reputation, governance, and the attestor whitelist.** Solana consumes
EVM-attested credit limits via short-TTL ed25519 signatures from a whitelisted
bridge signer, then handles advance issuance + settlement on Solana rails.

This split was the result of an architectural pivot on `evm-parity` (2026-05-06)
after recognizing that duplicating reputation + governance on Solana was both
expensive and a divergence-risk against the EVM lane.

## Read order (before touching code)

1. **`README.md`** — flow + workspace + commands.
2. The handler files themselves (`programs/credmesh-escrow/src/instructions/*.rs`,
   `programs/credmesh-attestor-registry/src/lib.rs`) — they cross-reference
   AUDIT findings inline as `// AUDIT P0-X` comments and document each
   invariant at the top of the relevant function.

> Maintainers: the audit / decisions / design / deployment / handoff
> internals live under `internal/` (gitignored). Ask a maintainer for
> access if you need historical rationale or the v1.5 hardening
> roadmap.

## Commands

```bash
# Toolchain (see CONTRIBUTING.md for full setup)
rustup default 1.79.0
sh -c "$(curl -sSfL https://release.anza.xyz/v1.18.26/install)"
cargo install --git https://github.com/coral-xyz/anchor avm --force && avm install 0.30.1

anchor build                                 # build both programs
cargo check --workspace                      # compile-only verification

# TS off-chain components
cd ts/server && npm install && npm run dev   # http://localhost:3000  (agent card + SIWS nonce)
cd ts/bridge && npm install && npm run dev   # http://localhost:4001  (EVM read + ed25519 sign)
cd ts/keeper && npm install && npm run dev   # liquidation crank
```

## Architecture

### Workspace layout

```
crates/
└── credmesh-shared/                 seeds, program IDs, helpers
                                     (cross_program, ix_introspection,
                                     ed25519_credit_message). Library only —
                                     never deployed.
programs/
├── credmesh-escrow/                 Pool vault + share-mint + advance + waterfall.
│                                    request_advance consumes ed25519 credit
│                                    attestations from a whitelisted bridge signer.
└── credmesh-attestor-registry/      Governance-controlled AllowedSigner PDA whitelist
                                     with kind tags (CreditBridge today, more later).
ts/server/                           Hono backend — agent card + SIWS nonce only.
ts/bridge/                           Off-chain EVM ⇒ Solana credit-attestation bridge
                                     (HTTP /quote endpoint; signs 128-byte ed25519
                                     credit messages after reading EVM
                                     ReputationCreditOracle + TrustlessEscrow).
ts/keeper/                           Liquidation crank for expired advances.
ts/shared/                           Shared TS constants (PDA seeds, ed25519 message
                                     offsets, anchor discriminator helpers).
```

NB: `credmesh-shared` lives in `crates/`, not `programs/`. Anchor traverses every
`programs/*` looking for `#[program]` modules; a library would error there.

### What's deliberately NOT here

- **`credmesh-reputation` was deleted in the pivot.** Reputation lives on EVM.
  The Solana side never re-derives a credit score. If you find yourself
  wanting to add a reputation field on-chain, that's a sign the EVM bridge
  attestation should carry it instead.
- **`credmesh-receivable-oracle` was renamed to `credmesh-attestor-registry`.**
  Receivables-as-PDAs are no longer a primitive — credit limit is the
  primitive, attested by the EVM bridge. The registry program just whitelists
  ed25519 signers.

### External programs CredMesh **uses** but does not deploy

- **Squads v4** (`SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf`) — protocol
  governance multisig (NOT agent vaults; agents are raw keypairs).
- **SPL Token classic** — USDC vault and share mint.
- **ed25519 program** + **Memo v2** — credit attestation verification +
  settlement nonce binding.

### EVM lane (single source of truth)

- **EVM `ReputationCreditOracle.maxExposure(agent)`** — agent's current credit
  limit in USDC atoms.
- **EVM `TrustlessEscrow.exposure(agent)`** — current outstanding balance
  across all chains where CredMesh runs (the bridge replays Solana settle/
  liquidate events back so this stays accurate).
- **EVM `IdentityRegistry`** — agent identity, attestation count, trust score.

## Conventions specific to this repo

- **Cross-program reads use the four-step verify** (owner → address →
  discriminator → typed deserialize) via
  `credmesh_shared::cross_program::read_cross_program_account<T>`. Forgetting
  any step is the Wormhole-class bug. **Don't skip steps.**
- **PDA seeds come from `credmesh-shared::seeds`.** Never re-declare seed
  bytes in two crates — they will silently drift.
- **`ConsumedPayment` is permanent** (AUDIT P0-5). Closing it reopens
  close-then-reinit replay. Don't add a close handler.
- **`request_advance` has no pause path.** Advance issuance is never gated by
  governance — only by ed25519 attestation freshness + signer whitelist
  membership. The "no pause on issuance" invariant is load-bearing — see
  DESIGN §3.5 and AUDIT P0-6.
- **`request_advance` consumes ed25519 credit attestations.** A
  whitelisted bridge signer (kind = `AttestorKind::CreditBridge` in
  `credmesh-attestor-registry`) signs a canonical 128-byte
  `ed25519_credit_message` (layout in `crates/credmesh-shared/src/lib.rs`).
  Handler verifies via instructions-sysvar introspection: prior ix is the
  ed25519 program, signer is in the registry, attestation is fresh
  (≤ 15 min), agent + pool match, version = 1. **Underwriting enforces**
  `available = attested_credit_limit - attested_outstanding` (mirrors EVM
  `availableCredit`).
- **`claim_and_settle` is single-mode** (post-pivot): agent self-settles.
  `agent: Signer` with `agent.key() == advance.agent`; two transfers
  (protocol cut to treasury, LP cut to vault); agent net stays in the agent
  ATA. Memo nonce binding preserved. Permissionless cranking via SPL
  `Approve` delegate was reverted in the pivot — the EVM bridge model means
  attestations are short-TTL and online, and the agent is reachable to
  self-settle within the window.
- **Three-key topology DROPPED.** No `oracle_worker_authority`, no
  `reputation_writer_authority`. The two on-chain trust roots are now:
  (1) the bridge ed25519 signer whitelist via `credmesh-attestor-registry`,
  (2) Squads governance over the registry + escrow `Pool.governance`.
  Compromise of a bridge key is bounded by the 15-min TTL on each
  attestation plus immediate revocation via `remove_allowed_signer`.
- **All math is `checked_*`** or wrapped in u128. Cargo.toml sets
  `overflow-checks = true` in release; don't rely on it.
- **Errors map to typed enums** (`CredmeshError`, `AttestorRegistryError`).
  No `unwrap()` in handlers.
- **Cross-program seed constants come from `credmesh-shared`.**
- **Events are emitted as the LAST step of each handler.** A partial failure
  mid-handler shouldn't emit a misleading event.
- **No `find_program_address` in hot paths** when the bump is already cached.
  Costs ~1500 CU per call. Use
  `Pubkey::create_program_address(seeds_with_bump, program_id)` instead.
- **Use `transfer_checked` not bare `transfer`.** All transfer call sites in
  credmesh-escrow have been migrated. anchor-spl 0.30.1 does NOT expose
  `mint_to_checked` or `burn_checked` — those stay as bare `mint_to` / `burn`
  (one site each: `deposit::handler` and `withdraw::handler`).

## What NOT to do

- Don't add a `paused` field back to `Pool`. The "no pause on issuance"
  invariant is load-bearing — AUDIT P0-6.
- Don't close `ConsumedPayment`. Permanent. AUDIT P0-5.
- Don't reintroduce a Solana-side reputation program. EVM is the single
  source of truth for credit scoring. Adding a reputation field on Solana
  divergence-risks the two lanes.
- Don't reintroduce a `Receivable` PDA primitive. Credit limit (EVM-attested)
  is the only credit primitive on Solana. The marketplace model is
  out-of-scope for v1.
- Don't introduce Light Protocol compressed PDAs or Token-2022 features in
  v1. Both are explicitly v2+.
- Don't use `init_if_needed` for replay-protection PDAs — only `init`.
  AUDIT P0-5.
- Don't trust client-supplied credit limits. Every advance must be
  underwritten against a fresh ed25519 attestation from a whitelisted
  bridge signer.

## V1 explicitly NOT in scope (deferred)

- Solana-native reputation scoring (EVM is canonical).
- Marketplace / receivable primitives on Solana.
- ML-derived credit curves.
- Mobile Wallet Adapter / Solana Mobile.
- Hyperliquid Lazer publisher.
- Light Protocol compressed PDAs.
- Multi-asset pools (USDC only).
- Per-instruction-type timelock granularity.
- Token-2022 USDC handling (Circle hasn't migrated).
- Embedded-wallet (Phantom Portal) auth.
- Permissionless `claim_and_settle` cranking — reverted in the EVM-bridge
  pivot. Agent self-settles within the 15-min attestation window's tolerance
  (settlement is invoked after the receivable lands, which is independent
  of the attestation lifetime).
- Multi-issuer SAS attestations (deferred to v1.5; schema documented now).
- Quorum-required bridge signers (any-valid-sig in v1; quorum is a v1.5
  hardening).

## Sister repo

The original EVM CredMesh lives at `../trustvault-credit/` (the EVM lane).
When porting math (e.g., the credit-limit formula in
`credit-worker/src/credit.ts`), the EVM file is the source of truth; the
Solana side just consumes the result via the bridge. **Do not re-port the
math; consume the attestation.**
