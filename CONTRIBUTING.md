# Contributing

Welcome. CredMesh Solana is in pre-implementation: research, design, and Anchor scaffolding are done; handler bodies are pending.

## Read first

1. [`DECISIONS.md`](./DECISIONS.md) — the 5 blocking design questions and their resolutions.
2. [`AUDIT.md`](./AUDIT.md) — security/account-model/integration findings; what was fixed and what's pending.
3. [`DESIGN.md`](./DESIGN.md) — implementer spec.
4. `research/` — supporting docs.

## Setup

```bash
# Rust toolchain
rustup toolchain install 1.79.0
rustup default 1.79.0

# Solana CLI
sh -c "$(curl -sSfL https://release.anza.xyz/v1.18.26/install)"

# Anchor
cargo install --git https://github.com/coral-xyz/anchor avm --force
avm install 0.30.1
avm use 0.30.1

# Verify
solana --version          # 1.18.26+
anchor --version          # 0.30.1
rustc --version           # 1.79.0+

# Build the workspace
anchor build

# TypeScript server
cd ts/server
npm install
npm run typecheck
```

## What to work on

The first sprint implements handler bodies. Pick one in priority order:

1. **`credmesh-escrow::init_pool`** — sets up Pool PDA, share mint, vault ATA, mints virtual-shares dead supply to Pool PDA itself for first-depositor defense. Self-contained; good first PR.
2. **`credmesh-escrow::deposit`** + **`credmesh-escrow::withdraw`** — straight u128 mul-div math + token CPI. Pair them.
3. **`credmesh-receivable-oracle::worker_update_receivable`** + **`add_allowed_signer`** — straightforward ACL'd writes.
4. **`credmesh-reputation::init_reputation`** + **`give_feedback`** — including the Q4 writer-gated `score_ema` update.
5. **`credmesh-escrow::request_advance`** (worker path first) — depends on cross-program deserialize helper. Add helper in `credmesh-shared` first.
6. **`credmesh-escrow::claim_and_settle`** — three-CPI waterfall with checked math; rounding remainder goes to agent.
7. **`credmesh-escrow::liquidate`** — keeps `Advance` alive with `state = Liquidated`.
8. **`credmesh-escrow::propose_params`** + **`execute_params`** — Squads CPI verification.
9. **ed25519 path on `request_advance`** — instruction-introspection helper in `credmesh-shared`. Includes asymmetric.re/Relay-class fix (verify offsets internal to verify ix).
10. **`request_advance` x402 path** — same as ed25519 but with `source_signer` allowlist gating.

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
- Don't make `claim_and_settle` permissionless in v1. Cranker must equal `advance.agent` until a payer-pre-auth pattern lands. See AUDIT P0-3 / P0-4.
- Don't introduce `Light Protocol compressed PDAs` or `Token-2022` features in v1. Both are explicitly v2+.
- Don't add per-record SQL persistence on the off-chain server. State migrates to on-chain PDAs (DESIGN §6); SQLite is a derived-view cache only.
