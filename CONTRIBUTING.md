# Contributing

The Solana lane of [CredMesh](https://github.com/unbrained-labs/credmesh).
See [`README.md`](./README.md) for what the protocol does and how the
on-chain + off-chain pieces fit together; see [`CLAUDE.md`](./CLAUDE.md)
for the coding conventions and don't-do list this repo enforces.

## Read first

1. [`README.md`](./README.md) — flow + workspace + commands.
2. [`CLAUDE.md`](./CLAUDE.md) — repo conventions (cross-program reads,
   PDA seeds, fee math, event-emit ordering, transfer_checked, etc.).
3. The handler sources themselves
   (`programs/credmesh-escrow/src/instructions/*.rs`,
   `programs/credmesh-attestor-registry/src/lib.rs`) — every load-bearing
   invariant is documented inline at the top of the relevant function.

## Toolchain

```bash
rustup default 1.79.0
sh -c "$(curl -sSfL https://release.anza.xyz/v1.18.26/install)"
cargo install --git https://github.com/coral-xyz/anchor avm --force && avm install 0.30.1

# TS workspaces are independent; install per-package as needed.
cd ts/shared && npm install
cd ts/server && npm install
cd ts/bridge && npm install
cd ts/keeper && npm install
```

## Daily commands

```bash
npm run check       # cargo check --workspace --locked
npm test            # cargo test --workspace --lib (16 pure-math + 2 program-id tests)
npm run typecheck   # tsc --noEmit across all four ts/ packages
npm run build       # anchor build (produces .so + IDL)
```

## Coding standards

- **Anchor 0.30** idioms throughout. `init`, `init_if_needed`, `mut`,
  `address`, `seeds`, `bump`, `has_one`, `constraint`. Read MarginFi v2
  or Drift's source for canonical patterns.
- **All math is `checked_*`** or wrapped in u128. Cargo.toml sets
  `overflow-checks = true` in release; don't rely on it.
- **Errors**: every fail path maps to a typed enum
  (`CredmeshError`, `AttestorRegistryError`). No `unwrap()` in handlers.
- **Cross-program reads** use the four-step verify (owner → address →
  discriminator → typed deserialize) via
  `credmesh_shared::cross_program::read_cross_program_account<T>`.
- **PDA seeds come from `credmesh-shared::seeds`.** Never re-declare
  seed bytes in two crates — they will silently drift.
- **`emit!` is the LAST step of every handler.** A partial failure
  mid-handler shouldn't emit a misleading event.
- **Use `transfer_checked`, not bare `transfer`** (Token-2022
  forward-compat). Bare `mint_to` / `burn` are still in two sites
  pending anchor-spl 0.30.1 shipping the checked wrappers.
- **Comments**: only for invariants, security notes, or non-obvious
  workarounds. Don't narrate what well-named code already says.

## What NOT to do

See [`CLAUDE.md`](./CLAUDE.md) "What NOT to do" section for the
load-bearing invariants. Highlights:

- Don't add a `paused` field to `Pool`. "No pause on issuance" is
  load-bearing.
- Don't close `ConsumedPayment`. Permanent.
- Don't reintroduce a Solana-side reputation program. EVM is canonical.
- Don't reintroduce a `Receivable` PDA primitive. Credit limit
  (EVM-attested) is the only credit primitive on Solana.
- Don't trust client-supplied credit limits. Every advance underwrites
  against a fresh ed25519 attestation from a whitelisted bridge signer.
- Don't use `init_if_needed` for replay-protection PDAs (only `init`).

## PR checklist

- [ ] `npm run check` passes (cargo)
- [ ] `npm test` passes (Rust unit tests)
- [ ] `npm run typecheck` passes (TS)
- [ ] `cargo fmt --all` clean
- [ ] `cargo clippy --workspace -- -D warnings` clean (warnings = errors
      for the deployable programs; the anchor-internal cfg-noise warnings
      are tolerated)
- [ ] If you touched `programs/credmesh-escrow/src/pricing.rs`, add or
      update a `#[cfg(test)] mod tests` test mirroring the change
- [ ] If you changed the on-chain ed25519 message layout in
      `crates/credmesh-shared/src/lib.rs::ed25519_credit_message`,
      bump `VERSION` and update the TS mirror in
      `ts/shared/src/index.ts` AND `ts/bridge/src/attestation.ts` in
      the same commit
