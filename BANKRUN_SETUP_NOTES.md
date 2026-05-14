# Bankrun harness — setup notes

This file scaffolds the bankrun test harness for the post-pivot
(`evm-parity` / PR #58 / `6317555`) repo. The harness boots and the test
is structurally wired end-to-end.

## Verified on 2026-05-13 (DaDo session)

- `anchor build` via `docker run backpackapp/build:v0.30.1` produces
  both `.so` files (252K + 508K). The build requires pinning
  `wasip2 -> 1.0.0` and `wit-bindgen -> 0.45.1` in `Cargo.lock` to
  sidestep the `edition2024` regression in the latest registry
  versions (cargo 1.79 can't parse them). The pin is committed in
  this PR.
- `cargo test --workspace --lib --locked` passes locally: 16 pricing
  tests + 3 program-id tests, 0 failures, 0 warnings of concern.
- `npm install && npm run test:bankrun` runs the harness but the
  T-CRY-08 test currently fails in the `before all` bootstrap at the
  `init_pool` ix with `Access violation in unknown section`
  (~CU 36744). The `init_registry` ix runs through clean. Likely
  cause: `event-cpi` is enabled on the workspace (`Cargo.toml` line
  19), which causes the `emit!` macro to expand into a self-CPI that
  needs two trailing accounts (`event_authority` PDA + program). I
  added a `eventCpiAccounts()` helper and appended those to
  `init_pool`, `init_registry`, and `request_advance` — `init_registry`
  now succeeds (it failed silently before), but `init_pool` still
  trips. Possible remaining causes:
    1. The event-cpi accounts need to be at a position other than the
       end of the accounts list for `init_pool` specifically.
    2. The `Pool::INIT_SPACE` allocation is too tight (look at
       `pending_params: Option<PendingParams>` — InitSpace for Option
       includes the discriminant byte; verify Anchor's macro produces
       the right total).
    3. A `mint::authority = pool` or `mint::freeze_authority = pool`
       constraint on `share_mint` may trigger a self-referential read
       before the pool's data is fully written.
  Use `cargo expand --manifest-path programs/credmesh-escrow/Cargo.toml`
  inside the docker container to inspect the macro output and confirm
  which struct fields get auto-injected by `event-cpi`.

## Toolchain summary (the path that worked)

```
# Local cargo test (16 + 3 unit tests):
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.79.0 --profile minimal
source $HOME/.cargo/env
cargo test --workspace --lib --locked

# Anchor build via docker (avoids the edition2024 cargo regression):
docker pull backpackapp/build:v0.30.1
# One-time lockfile pin to sidestep edition2024 in transitive deps:
docker run --rm -v "$(pwd):/workdir" -w /workdir backpackapp/build:v0.30.1 \
  bash -c "cargo update -p wasip2 --precise 1.0.0"
docker run --rm -v "$(pwd):/workdir" -w /workdir backpackapp/build:v0.30.1 anchor build

# Run the bankrun harness:
npm install
npm run test:bankrun
```

## Files added

- `package.json` — new devDeps (`@coral-xyz/anchor@0.30.1`,
  `solana-bankrun@^0.4.0`, `anchor-bankrun@^0.5.0`, `ts-mocha`, `chai`,
  `@types/{chai,mocha}`, `@solana/web3.js`, `@solana/spl-token`,
  `tweetnacl`) + npm script `test:bankrun`.
- `tsconfig.json` — added `programs/**/tests/**/*` to `include`.
- `programs/credmesh-escrow/tests/helpers/setup.ts` — bootstrap. Loads
  `.so` via `anchor-bankrun.startAnchor(workspaceRoot)`, mints a fresh
  USDC, funds deployer/LP/agent_a/agent_b wallets, inits the
  attestor-registry config, inits a pool with sane defaults
  (CHAIN_ID_DEVNET, fee curve from `scripts/init_pool.ts`,
  `max_advance_abs = 100 USDC`, `agent_window_cap = 0`).
- `programs/credmesh-escrow/tests/helpers/ed25519.ts` — encodes the
  canonical 128-byte attestation matching
  `crates/credmesh-shared/src/lib.rs::ed25519_credit_message`, signs
  with tweetnacl, builds the verify ix via
  `Ed25519Program.createInstructionWithPublicKey` (offsets reference
  the verify ix itself, which is what `verify_prev_ed25519` requires).
- `programs/credmesh-escrow/tests/helpers/memo.ts` — Memo v2 ix carrying
  a 16-byte nonce; on-chain check is byte-for-byte (`ix.data == nonce`,
  no length prefix).
- `programs/credmesh-escrow/tests/request_advance.test.ts` — T-CRY-08
  cross-agent replay fixture. Bridge signs an attestation bound to
  `agent_a`; `agent_b` tries to consume it; expects revert with
  `Ed25519MessageMismatch` (code 0x1778 = 6008, the 9th variant of
  `CredmeshError`).

## Commands run

```bash
# Toolchain probe (all 3 missing on this machine)
which anchor cargo rustc
# → all "not found"

# JS deps install — OK
npm install --no-audit --no-fund
# 227 packages added in 3s (npm WARN EBADENGINE for node@20.10 vs >=20.18 on
# the @solana/codecs-* 2.x line — does not block the bankrun harness)

# Typecheck — OK (no errors)
npx tsc --noEmit -p tsconfig.json

# Test run — BLOCKED at startAnchor (no .so files):
npm run test:bankrun
```

## Blocker

`startAnchor(workspaceRoot)` panics inside solana-program-test:

```
WARN  solana_program_test] No SBF shared objects found.
thread 'tokio-runtime-worker' panicked at solana-program-test/src/lib.rs:716:17:
Program file data not available for credmesh_attestor_registry
  (ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk)
```

`target/deploy/` currently contains only the program-id keypairs
(`credmesh_attestor_registry-keypair.json`, `credmesh_escrow-keypair.json`
— the attestor-registry keypair was renamed from
`credmesh_receivable_oracle-keypair.json` in commit `fafed26`). The
matching `.so` artifacts must be produced by `anchor build` on a machine
with Rust 1.79 + Solana 1.18.26 + Anchor 0.30.1 installed (see
`CONTRIBUTING.md`).

## Verifying on a machine with the toolchain

```bash
# Build .so files
anchor build

# Run only the bankrun suite
npm run test:bankrun
```

Expected output: 1 passing test under `request_advance — adversarial`.

If the test fails with a different on-chain error code than `0x1778`,
suspect: (a) `AllowedSigner` PDA pre-stamp owner pubkey mismatch, (b)
attestation `chain_id` not equal to `pool.chain_id` (the test uses
`CHAIN_ID_DEVNET = 2` for both, matching `init_pool.rs`).

## Gotchas a future contributor should know

1. **No IDL** — Anchor 0.30 IDL extraction is blocked behind issue #15.
   The test helpers hand-roll Borsh encoders against the Rust structs,
   matching the convention in `scripts/`. If `target/idl/` ever exists,
   the typed `Program<T>` client can replace the encoders; until then,
   any field-order change in `InitPoolParams` / `request_advance` args
   must update three sites: the Rust struct, the script, and the test
   encoder.

2. **Pre-stamping `AllowedSigner` via `setAccount`** — the test sidesteps
   the Squads-CPI governance gate on `add_allowed_signer` by writing the
   PDA's bytes directly into bankrun's ledger. The account layout is
   `discriminator(8) || bump(u8) || signer(32) || kind(u8) || added_at(i64)`
   = 50 bytes, and the owner MUST be the attestor-registry program id
   (otherwise Anchor's `Account<T>` deserialize will reject on owner
   check). The discriminator is `sha256("account:AllowedSigner")[..8]`.

3. **ed25519 verify ix must be IMMEDIATELY before `request_advance`** —
   `verify_prev_ed25519` reads `cur_idx - 1`. Inserting a compute-budget
   ix between them is a footgun; in the T-CRY-08 fixture the
   compute-budget ix is appended AFTER `request_advance` to preserve
   adjacency.

4. **`attested_at` freshness** — bankrun's clock starts at the host's
   wall time, so `Date.now()/1000` works as `attested_at`. If a future
   test fast-forwards the bankrun clock (`context.warpToSlot` etc.),
   `attested_at` must be recomputed to stay inside the 15-min
   `MAX_ATTESTATION_AGE_SECONDS` window.

5. **`chain_id` mismatch silently maps to `InvalidChainId`, not
   `Ed25519MessageMismatch`** — a mistake in either the attestation or
   `init_pool` chain_id surfaces as a *different* error code (0x178c =
   6028 vs 0x1778 = 6008). When extending the suite, prefer asserting
   the specific code, not "any revert."

6. **The freshly-init'd `usdcMint` is not mainnet USDC** — Anchor.toml
   clones mainnet USDC for `solana-test-validator`, but bankrun is its
   own universe. Tests that want mainnet USDC's exact pubkey (e.g., to
   exercise the bridge's chain-tracking semantics) must use bankrun's
   `extraAccounts` parameter to `startAnchor` to clone the mainnet
   account by pubkey + state, not rely on the cluster clone.

## Worktree info

- Path: `/Users/danieldo/repos/unbrainedORG/credmesh-solana/.claude/worktrees/agent-a46b32f97d563b107`
- Branch: `worktree-agent-a46b32f97d563b107`
- Base: `main` @ `ef95e76`
