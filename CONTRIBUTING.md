# Contributing

Welcome. CredMesh-Solana v1 is implemented, audited, and partially deployed to devnet. See `README.md` for current status.

## Read first

1. [`docs/ARCHITECTURE.md`](./docs/ARCHITECTURE.md) and [`docs/LOGIC_FLOW.md`](./docs/LOGIC_FLOW.md) — Mermaid diagrams of the static structure + handler sequences.
2. [`DECISIONS.md`](./DECISIONS.md) — the 5 blocking design questions and their resolutions.
3. [`AUDIT.md`](./AUDIT.md) — security findings, what was fixed (incl. post-EPIC #9 audit-driven fixes), and what's pending.
4. [`DESIGN.md`](./DESIGN.md) — implementer spec (kept in sync with current code).
5. [`DEPLOYMENT.md`](./DEPLOYMENT.md) — Docker build recipe + deploy procedure + key rotation.
6. [`V1_ACCEPTANCE.md`](./V1_ACCEPTANCE.md) — gating checklist for mainnet.
7. `research/` — original research artifacts (some superseded; see CONTRARIAN.md for what we redesigned).

## Setup

The pinned Anchor 0.30.1 + Solana 1.18.26 toolchain has lockfile-drift issues against modern Cargo registry contents. **The canonical build is Docker** — don't try to install the toolchain natively. Full recipe in `DEPLOYMENT.md § Build environment (Docker)`. Quickstart:

```bash
# Pre-warm cached Docker volumes (one-time):
docker pull backpackapp/build:v0.30.1
docker volume create credmesh-rustup
docker volume create credmesh-cargo-registry
docker volume create credmesh-cargo-git
docker volume create credmesh-pt-cache
docker run --rm -v credmesh-rustup:/root/.rustup backpackapp/build:v0.30.1 \
  rustup toolchain install 1.86.0 --profile minimal --no-self-update

# Build the workspace (the wrapper script in DEPLOYMENT.md injects
# --tools-version v1.50 so cargo-build-sbf doesn't re-install platform-tools).
# Use `--no-idl` until issue #15 lands; the deployable .so still produces correctly.

# TypeScript tests + server (host-side, no Docker needed)
npm install
npm test           # ts-mocha + anchor-bankrun (pure-math suites run today)
cd ts/server && npm run typecheck
```

If you must install the toolchain natively (NOT recommended), see CONTRIBUTING_NATIVE.md (TODO if anyone asks). Otherwise stick to Docker; everything contributors do works that way.

## What to work on

V1 handlers are implemented + compiled + (mostly) deployed. The active gaps are:

1. **Issue #15: IDL extraction (E0433 on `AssociatedToken`)** — biggest unlock; activates the harness-mode bankrun tests + TS-client typed-tx + Codama. One-line fix likely in `programs/credmesh-escrow/src/lib.rs:~1006` (add `use anchor_spl::associated_token::AssociatedToken;`); needs a clean rebuild + IDL regen.
2. **`credmesh-escrow` deploy** — keypair reserved at `DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF`; needs ~3.5 SOL on the deployer wallet.
3. **Squads CPI verification on `propose_params`** — currently a `Signer<'info>` constraint; Squads vault PDAs cannot be Signers. Needs an on-chain ix-introspection helper that verifies the ix is `vault_transaction_execute` from the Squads program. Tracked in `DEPLOYMENT.md § Phase 3`.
4. **Promote bankrun harness scaffolds to live behavioral tests** — currently `expect(true).to.be.true` placeholders pending the IDL fix. Activate once #15 lands.
5. **`ts/server` route handlers** — SIWS auth middleware works; `POST /agents/:address/advance`, the Helius webhook ingest, and the Codama-generated client are all stubs.
6. **`ts/dashboard`** — empty. React 19 + Vite + Tailwind + Phantom Connect; SSE-relayed `accountSubscribe` for live timeline.
7. **Insurance fund / first-loss tranche** (`InsuranceFund` PDA, 5% TVL) — gating LP recruitment.

If you're picking up new work, run an `anchor build --no-idl` first to confirm your local toolchain matches the Docker recipe; otherwise the build will fail on the same drift Track A spent 4 hours debugging.

## Coding standards

- **Anchor 0.30** idioms throughout. `init`, `init_if_needed`, `mut`, `address`, `seeds`, `bump`, `has_one`, `constraint`. Read MarginFi v2 or Drift's source for canonical patterns.
- **All math is `checked_*`** or wrapped in u128. `cargo.toml` already sets `overflow-checks = true` in release; don't rely on it.
- **Errors**: every fail path maps to a typed `CredmeshError`/`ReputationError`/`OracleError`. No `unwrap()` in handlers.
- **No comments that narrate the obvious.** Comments are for invariants, security notes, and AUDIT/DECISIONS cross-references.
- **Cross-program seed constants come from `credmesh-shared`**. Never re-declare seed bytes in two crates.
- **Events are emitted as the LAST step of each handler** (so a partial failure doesn't emit a misleading event).
- **Add or update a Bankrun test for every handler change.** Attacks → fixture in `tests/bankrun/attacks/` proving the fix.

## PR checklist

- [ ] `anchor build` passes
- [ ] `cargo fmt --all` clean
- [ ] `cargo clippy --workspace -- -D warnings` clean
- [ ] Bankrun tests added/updated for changed handlers
- [ ] AUDIT.md / DECISIONS.md updated if a finding/decision changed
- [ ] DESIGN.md updated if the spec moved

## What NOT to do

- Don't add the `paused` flag back to `Pool`. The "no pause on issuance" invariant is load-bearing — see DESIGN §3.5 / AUDIT P0-6.
- Don't close `ConsumedPayment`. It must be permanent — closing reopens replay. See AUDIT P0-5.
- `claim_and_settle` is two-mode (DECISIONS Q9, supersedes AUDIT P0-3 / P0-4 deferral). Mode A: cranker == advance.agent (legacy v1, preserved). Mode B: any signer; pool PDA is the SPL `Approve` delegate on `agent_usdc_ata` (granted by `request_advance`). Source-of-funds and destination-of-funds ATAs are pinned per-account, so cranker identity is not load-bearing for safety. See `research/CONTRARIAN-permissionless-settle.md`.
- Don't introduce `Light Protocol compressed PDAs` or `Token-2022` features in v1. Both are explicitly v2+.
- Don't add per-record SQL persistence on the off-chain server. State migrates to on-chain PDAs (DESIGN §6); SQLite is a derived-view cache only.
