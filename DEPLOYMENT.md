# DEPLOYMENT

How to deploy CredMesh Solana to devnet and mainnet-beta. **All steps assume v1 acceptance criteria are met (see [V1_ACCEPTANCE.md](./V1_ACCEPTANCE.md)).**

## Prerequisites

- Solana CLI 1.18.26+
- Anchor 0.30.1
- Rust 1.79.0+ (host) — but in practice cargo 1.84+ is required to resolve
  the modern transitive dep graph; see "Toolchain pins" below
- A funded deployer wallet (~3 SOL devnet, ~10 SOL mainnet for one-time costs)
- A pre-created Squads v4 governance multisig (mainnet only)

## Toolchain pins (Anchor 0.30 + Solana 1.18 lockfile drift)

The Anchor 0.30 / Solana 1.18 ecosystem was current in mid-2024. By 2026 the
crates.io graph has drifted: several transitive deps now require cargo
features (`edition2024`, stable `--check-cfg`) that the era-correct toolchain
does not understand. Three pins live in this repo to keep the build
reproducible without forcing an Anchor 0.31 upgrade:

| Pin | Where | Reason |
|-----|-------|--------|
| `blake3 = "=1.5.5"` | `crates/credmesh-shared/Cargo.toml` | Newer blake3 (≥1.6) pulls `digest 0.11` → `crypto-common 0.2.1`, whose Cargo manifest requires Cargo's `edition2024` feature (stabilized in Rust 1.85). blake3 is already a transitive of `solana-program 1.18.26`, so this only constrains the version, not the binary footprint. |
| `proc-macro2 = "=1.0.86"` | `crates/credmesh-shared/Cargo.toml` | `anchor-syn 0.30.1` calls `proc_macro2::Span::source_file()` during IDL build. proc-macro2 ≥1.0.92 renamed it to `local_file()`. 1.0.86 is contemporaneous with anchor 0.30.1 (both June 2024). |
| `[workspace.package] rust-version = "1.75"` | `Cargo.toml` (root) | Combined with `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback`, makes the resolver pick crate versions whose own `rust-version` matches the BPF toolchain rustc, instead of pulling versions that need Rust 1.85+. |

**Anchor 0.30 syntax fixes** that landed bringing up the build:
- `idl-build` feature on every workspace member that gets traversed by
  `anchor build` (even leaf libs)
- `associated_token` feature on `anchor-spl` for the escrow program (it
  references `anchor_spl::associated_token::AssociatedToken`)
- `credmesh-shared` lives under `crates/`, **not** `programs/` — Anchor
  treats every `programs/*` subdir as a deployable program and tries to
  extract an IDL from it. A library with no `#[program]` mod can't satisfy
  that.
- `credmesh-shared` is `crate-type = ["lib"]` only (was `["cdylib", "lib"]`),
  so anchor doesn't try to BPF-link a non-program.

## Build environment (Docker)

The recipe that produces a green build on a fresh machine. The image's own
toolchain (`solana 1.18.17` + `platform-tools v1.41` + `rustc 1.75-dev`) is
too old for `cargo 1.84+`'s stable-`--check-cfg` flag, so we override two
pieces of it: install Rust 1.86 host cargo into a named volume, and tell
`cargo-build-sbf` to download `platform-tools v1.50` (rustc 1.84.1) instead
of its bundled v1.41.

One-time setup:

```bash
docker pull backpackapp/build:v0.30.1
docker volume create credmesh-rustup
docker volume create credmesh-cargo-registry
docker volume create credmesh-cargo-git
docker volume create credmesh-pt-cache
# Pre-warm the host toolchain volume:
docker run --rm -v credmesh-rustup:/root/.rustup backpackapp/build:v0.30.1 \
  rustup toolchain install 1.86.0 --profile minimal --no-self-update
```

Build:

```bash
docker run --rm \
  -v "$PWD:/workdir" \
  -v credmesh-rustup:/root/.rustup \
  -v credmesh-cargo-registry:/root/.cargo/registry \
  -v credmesh-cargo-git:/root/.cargo/git \
  -v credmesh-pt-cache:/cache \
  -e RUSTUP_TOOLCHAIN=1.86.0 \
  -e CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback \
  -e RUSTC_BOOTSTRAP=1 \
  -w /workdir \
  backpackapp/build:v0.30.1 \
  bash -c '
    set -e
    REAL=/root/.local/share/solana/install/active_release/bin/cargo-build-sbf
    mkdir -p /opt/wrapper
    cat > /opt/wrapper/cargo-build-sbf <<EOF
#!/bin/bash
# Cargo invokes us as: argv = [cargo-build-sbf, build-sbf, ...user args].
# Clap inside the real binary auto-strips argv[1] iff it equals the
# subcommand suffix, so we must preserve build-sbf in that slot and inject
# --tools-version AFTER it.
if [ "\$1" = "build-sbf" ]; then
  exec "$REAL" "\$1" --tools-version v1.50 "\${@:2}"
else
  exec "$REAL" --tools-version v1.50 "\$@"
fi
EOF
    chmod +x /opt/wrapper/cargo-build-sbf
    export PATH=/opt/wrapper:$PATH
    anchor build --no-idl
  '
```

**Known limitation:** drop `--no-idl` and `anchor build` fails during the
IDL extraction pass with `error[E0433]: failed to resolve: use of undeclared
type AssociatedToken` at `programs/credmesh-escrow/src/lib.rs:1006`, even
though `cargo tree -e features -p credmesh-escrow --features idl-build`
confirms the feature is active. Tracked as a follow-up to issue #7. The
deployable artifact is the `.so`, which `--no-idl` produces; the IDL JSON
is needed for Codama client gen on Day 2.

## Phase 0 — Generate program IDs

The placeholder IDs in source (`11111111111111111111111111111112` /
`…3` / `…4`) must be replaced with deterministic deploy keypairs.

```bash
mkdir -p target/deploy
solana-keygen new --no-bip39-passphrase -o target/deploy/credmesh_escrow-keypair.json
solana-keygen new --no-bip39-passphrase -o target/deploy/credmesh_reputation-keypair.json
solana-keygen new --no-bip39-passphrase -o target/deploy/credmesh_receivable_oracle-keypair.json

# Read the public keys
solana-keygen pubkey target/deploy/credmesh_escrow-keypair.json
solana-keygen pubkey target/deploy/credmesh_reputation-keypair.json
solana-keygen pubkey target/deploy/credmesh_receivable_oracle-keypair.json
```

`anchor keys sync` will rewrite the `declare_id!` lines and `Anchor.toml`
automatically. The `program_ids::*` consts in `crates/credmesh-shared/src/lib.rs`
must be updated by hand (they're application-level constants, not
Anchor-managed). Manually mirror `[programs.localnet]` to
`[programs.devnet]` after `anchor keys sync` — by default it only updates
the active cluster.

The three deployable program keypairs are committed to the repo (see
`.gitignore` exception); `credmesh-shared` is a library and has no
deployable keypair.

## Phase 1 — Devnet deploy

The deploy machinery (Track A, PR #16) is in place; running it requires
three org-level inputs that are deliberately not in the repo: a governance
pubkey, a worker authority pubkey, and a treasury USDC ATA. The Day-3
operational notes below are written for the operator who has those three
pubkeys in hand.

### Inputs (all TBD — fill in before running)

| Input | What it is | Where to get it |
|-------|-----------|-----------------|
| `--governance` | A Squads v4 vault PDA — must NOT equal the deployer wallet (DESIGN §10) | Provision a Squads v4 multisig on devnet via the Squads UI. The "vault PDA" is one of the addresses Squads exposes per multisig — copy it from the Squads config view. Same address can be used for the oracle and the pool. |
| `--worker-authority` | The hot key the off-chain worker signs `worker_update_receivable` with. MUST NOT equal governance or the reputation writer (DESIGN §10 three-key topology). | `solana-keygen new -o ~/.credmesh/worker-authority.json` then `solana-keygen pubkey ~/.credmesh/worker-authority.json`. Keep this key on whatever host the worker runs on; rotate quarterly via `set_worker_authority`. |
| `--treasury-ata` | The USDC ATA owned by the protocol treasury wallet on devnet. Where `claim_and_settle` deposits the protocol fee share. | The treasury wallet is a separate Squads vault from the governance one. Use `spl-token create-account 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU --owner <treasury-vault-pda> --url devnet` to mint the ATA, or derive deterministically with `getAssociatedTokenAddressSync` against the devnet USDC mint and have the deployer fund 0.002 SOL of rent. |

### Funding the deployer wallet

Three programs (`escrow`, `reputation`, `receivable-oracle`) plus the init
flow's fresh share-mint, USDC vault, and OracleConfig PDA. Conservative
upfront budget for devnet: **~10 SOL**. Devnet airdrops cap at 5 SOL per
call and rate-limit aggressively, so plan two airdrops with a minute or
two between them.

```bash
# Generate a deployer wallet — interactive passphrase prompt; the
# --no-bip39-passphrase flag suppresses the seed-phrase passphrase prompt
# but you still set a wallet passphrase (use empty string for devnet ops).
solana-keygen new --no-bip39-passphrase -o ~/.config/solana/id.json

# Two airdrops to cover program rent (~7 SOL across the three .so files)
# plus init-flow rent + tx fees.
solana airdrop 5 -u devnet
sleep 90  # rate-limit cooldown
solana airdrop 5 -u devnet
solana balance -u devnet  # should be ~10 SOL
```

If `solana airdrop` returns `Error: airdrop request failed`, the public
faucet is throttled — fall back to the [web faucet](https://faucet.solana.com)
or one of the third-party devnet faucets (Helius, QuickNode).

### Build, then deploy

```bash
# 1. Build BPF artifacts (Docker recipe; see "Build environment" above).
#    The `--no-idl` flag is required until issue #15 lands.
#    Output: target/deploy/credmesh_{escrow,reputation,receivable_oracle}.so

# 2. Install root JS deps if you haven't yet.
npm install

# 3. Deploy all three programs to devnet.
npm run deploy -- --cluster devnet --program all
```

`scripts/deploy.ts` wraps `solana program deploy` per program. It is
idempotent — re-running on an already-deployed program-id issues a BPF
Loader Upgradeable v3 upgrade against the same address. Output includes
the local sha256 of each `.so` (a verifiable build hash) and a post-deploy
RPC `getAccountInfo` to confirm the program-id is live.

### Initialize state

```bash
# 1. OracleConfig (worker authority + governance + caps).
npm run init:oracle -- \
  --cluster devnet \
  --governance <SQUADS_VAULT_PDA> \
  --worker-authority <WORKER_HOT_KEY_PUBKEY> \
  --worker-max-per-tx 100000000 \
  --worker-max-per-period 10000000000 \
  --period-seconds 86400

# 2. Pool (USDC mint, fee curve, caps, treasury ATA).
npm run init:pool -- \
  --cluster devnet \
  --asset-mint 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU \
  --governance <SQUADS_VAULT_PDA> \
  --treasury-ata <TREASURY_USDC_ATA> \
  --max-advance-pct-bps 3000 \
  --max-advance-abs 100000000 \
  --timelock-seconds 86400
```

Both scripts refuse to re-init when the target PDA already holds account
data, so a half-finished init is safe to re-attempt — fix the failed step,
re-run only that step. Order matters: oracle must exist before pool (the
pool relies on the oracle for receivable lookups, but the dependency is
runtime, not init-time — strictly speaking either order works for the init
itself; conventional is oracle-first).

`init_oracle.ts` enforces `governance != worker_authority` client-side
before sending. `init_pool.ts` enforces `governance != deployer`.

### Verify

```bash
# Confirm each program-id is live and points at our keypair-derived addresses.
solana program show DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF -u devnet  # escrow
solana program show JDBeDr9WFhepcz4C2JeGSsMN2KLW4C1aQdNLS2jvc79G -u devnet  # reputation
solana program show ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk -u devnet  # oracle

# `anchor verify` requires the IDL — defer until #15 lands. Until then,
# the local sha256 (printed by `scripts/deploy.ts`) is the verifiable
# build hash; cross-reference with the on-chain program-data account if
# you need stronger guarantees.
```

### Day 3 — operational notes (gotchas)

What was learned across Track A Days 1-2 that the next operator should
know before they hit the same walls:

**Build environment**

- *The `linux/amd64` platform warning is harmless on Apple Silicon.* Docker
  Desktop runs the image under Rosetta. First build is ~5-10 min cold and
  ~1-2 min warm with the four named volumes (`credmesh-rustup`,
  `credmesh-cargo-registry`, `credmesh-cargo-git`, `credmesh-pt-cache`)
  populated — never `docker volume rm` these unless you want to pay the
  cold-cache cost again.
- *The `cargo-build-sbf` wrapper is required even for dev builds.* The
  one-liner shell wrapper that injects `--tools-version v1.50` lives at
  `/opt/wrapper/cargo-build-sbf` inside the container; it must be on the
  PATH _before_ the bundled `solana/install/active_release/bin` directory.
  Forgetting the PATH order is silent — cargo-build-sbf falls back to its
  bundled `platform-tools v1.41` (rustc 1.75-dev) which then fails on
  cargo 1.86's stable `--check-cfg` flag.
- *The wrapper's argv handling matters.* Cargo invokes the binary with
  `argv[1]="build-sbf"` (the subcommand name); clap inside the real binary
  auto-strips that slot only if the binary is named `cargo-build-sbf` and
  argv[1] is intact. The wrapper preserves both — if you rewrite the
  wrapper to put `--tools-version v1.50` _before_ the subcommand arg, the
  real binary errors with `Found argument 'build-sbf' which wasn't
  expected`.
- *`anchor build --no-idl` is the canonical green build for now.* Drop the
  flag and you hit issue #15 (`AssociatedToken` resolution under cargo's
  `--features idl-build` test profile). The `.so` artifacts are identical
  either way — the `--no-idl` path just skips the post-compile IDL JSON
  emission.
- *Cargo.lock is committed.* Our three transitive-dep pins (blake3,
  proc-macro2, workspace MSRV) live in Cargo.toml; the lockfile makes them
  reproducible across machines. If Cargo.lock disappears, re-running the
  Docker build with `RUSTUP_TOOLCHAIN=1.86.0` and
  `CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback` reconstructs an
  equivalent lockfile from the same Cargo.toml constraints.
- *`anchor keys sync` only updates the active cluster's `[programs.X]`.*
  The `[provider].cluster = "Devnet"` in our Anchor.toml means devnet gets
  rewritten automatically; localnet is mirrored by hand. If you ever flip
  `[provider].cluster` to "Localnet" and re-run `anchor keys sync`,
  re-mirror after.
- *`anchor keys sync` auto-generates a `target/deploy/credmesh_shared-keypair.json`*
  even though `credmesh-shared` is a library. The `.gitignore` carves out
  the three deployable program keypairs but explicitly excludes the
  `credmesh_shared` one — don't add it back.

**Deploy mechanics**

- *`solana program deploy` is upgrade-on-rerun.* It looks up the
  program-id, sees the program already exists, and issues an upgrade
  using the upgrade authority (the deployer wallet by default). To
  redeploy from scratch, first `solana program close <PROGRAM_ID>`,
  reclaim the rent, then redeploy.
- *Program rent is non-trivial.* The current `.so` sizes (461 KB / 296 KB
  / 241 KB) consume ~3.2 / 2.1 / 1.7 SOL respectively under the BPF Loader
  Upgradeable v3 rent model. A fresh-wallet devnet bring-up needs ~10 SOL
  total, not the README's quoted 3 SOL.
- *Devnet airdrops rate-limit hard.* Two `solana airdrop 5 -u devnet`
  calls a minute apart usually work; in busy hours you may need the
  [web faucet](https://faucet.solana.com) or a third-party (Helius,
  QuickNode). Keep the deployer wallet topped up — out-of-funds during a
  multi-program deploy leaves a partial deploy that requires
  `solana program close` to clean up.
- *Pool PDA is keyed on the asset mint.* `seeds = ["pool", asset_mint]`.
  Devnet USDC and mainnet USDC have different mints, so the pool PDA on
  one cluster does not match the other. If you ever switch the asset mint
  (e.g., if Circle redeploys USDC), you cannot migrate the existing pool —
  init a new one.
- *`init_pool.ts` generates fresh share-mint and USDC-vault keypairs.*
  Both are passed as transaction signers alongside the deployer. The
  pubkeys are throwaway — the program owns both accounts after init.
  Don't try to reuse a previous run's pubkeys; they'll fail the `init`
  constraint.
- *The hand-rolled `init_pool.ts` discriminator is `sha256("global:init_pool")[0..8]`.*
  When issue #15 lands and the escrow IDL becomes available, replace the
  hand-rolled instruction builder with a typed `Program` (mirror
  `init_oracle.ts`); the discriminator and Borsh layout will match
  exactly, but the typed version catches account-order bugs at compile
  time.

**Anchor 0.30 specifics**

- *`[features] resolution = true` in Anchor.toml auto-resolves PDA accounts.*
  When calling typed `program.methods.foo().accounts({...}).rpc()`, do NOT
  pass `config: configPda` for an account that has `pda: { seeds: [...] }`
  in the IDL. Anchor's TS code derives it. Passing it manually is a
  TypeScript error (good) but if you cast to `any` you'll get a runtime
  account-mismatch error (bad).
- *`system_program` is also auto-resolved.* Same as PDAs.
- *Program constructor takes the IDL JSON directly.* Anchor 0.30 deduces
  the program-id from `idl.address`. The hand-spread of
  `RECEIVABLE_ORACLE_PROGRAM_ID.toBase58()` over `idl.address` in
  `init_oracle.ts` is defensive in case the extracted IDL was generated
  under a different keypair (it shouldn't be, but being explicit costs
  nothing).
- *`target/types/<program>.ts` is just a TypeScript type alias.* It is
  not a runtime artifact — `import type` is the correct import. The
  runtime IDL is the JSON.

**Wallet + key management**

- *The three-key topology (DESIGN §10) is not a guideline — it is enforced.*
  `init_oracle.ts` rejects `governance == worker_authority` client-side
  before sending; `init_pool.ts` rejects `governance == deployer`. The
  reputation writer authority defaults to governance until
  `set_reputation_writer` is called — once the worker is live in prod,
  rotate it to a separate key.
- *The deployer wallet is also the upgrade authority by default.*
  Phase 3 transfers it to a Squads vault; until then, the deployer's
  hot key controls program upgrades. Keep that key cold or rotate.
- *Wallet path expansion supports `~`.* `scripts/lib/cluster.ts`
  expands `~` → `$HOME` so `--wallet ~/.config/solana/id.json` works; bare
  paths are resolved against `$PWD`.

**Recovery**

- *Half-init recovery: just re-run the failing step.* Both init scripts
  refuse to re-init existing PDAs (refuse-then-exit-2). If `init_oracle`
  succeeded but `init_pool` failed, fix the pool args and re-run only
  `init_pool` — the oracle config is already in place and is single-shot
  anyway.
- *Half-deploy recovery: `solana program close <PROGRAM_ID>`.* This
  reclaims the rent and frees the program-id for redeploy. Lose the
  upgrade authority and the program is permanently un-upgradeable, so
  don't `--bypass-warning` flags carelessly.
- *Failed `init_pool` doesn't leave orphan accounts in the common case.*
  Anchor's `init` constraint reverts the whole tx atomically on any
  failure. The fresh share-mint and USDC-vault keypairs that
  `init_pool.ts` generates are throwaway and the SOL stays with the
  deployer.

## Phase 2 — Mainnet-beta staging

**Do NOT skip phase 1.** Devnet must run for ≥7 days with synthetic load before mainnet flip (V1_ACCEPTANCE gate).

```bash
solana config set --url mainnet-beta
# Deploy with the SAME keypairs as devnet — Squads governance assumes
# program-ID continuity. (Or use distinct keypairs and update the registry.)
anchor deploy --provider.cluster mainnet
```

### Mainnet hard caps (initial)

- `max_advance_pct_bps = 3000` (30% of receivable)
- `max_advance_abs = 100_000_000` ($100)
- Insurance buffer ≥ 5% of vault TVL pre-seeded by protocol treasury

## Phase 3 — Transfer authority to Squads

Critical: program upgrade authority must move to the CredMesh governance Squads vault before any mainnet TVL is allowed to grow past staging caps.

```bash
solana program set-upgrade-authority <ESCROW_ID> \
  --new-upgrade-authority <CredMesh governance Squads vault PDA> \
  --url mainnet-beta

# Repeat for credmesh_reputation, credmesh_receivable_oracle.
```

After this, the deployer key has **no further authority** over the programs. All upgrades go through the Squads multisig + timelock.

## Phase 4 — Bridge USDC into protocol treasury

```bash
# Coinbase Onramp or CCTP V2 from Base USDC → Solana USDC.
# Land into the governance-controlled treasury ATA.
```

## Phase 5 — Activate

- Onboard first agent via the CredMesh dashboard (`POST /agents/:address`).
- Verify SIWS auth flows.
- Allow first advance ≤ $10.
- Watch Helius webhook event stream for `AdvanceIssued`.
- Settle within ~1 hour to test full lifecycle.

## Rotation procedures

### Rotate worker authority (oracle key)

1. Squads governance proposes `update_oracle_config { worker_authority: NEW_KEY }`.
2. Wait `timelock_seconds`.
3. Squads executes. Old key is no longer accepted by `worker_update_receivable`.

### Rotate reputation writer

Same pattern via the OracleConfig field.

### Rotate fee-payer (PayAI ↔ self-host Kora)

Off-chain only — change the `PAYAI_FACILITATOR_URL` env var on the server. No on-chain change.

### Pause issuance (NOT IMPLEMENTED)

CredMesh's `request_advance` is **never gated by governance** (DESIGN §3.5). There is no kill-switch. Stopping issuance requires either:

(a) Withdrawing oracle worker authority — receivables stop being written, so caps return zero. Effective for 24-48 hours until existing receivables expire.
(b) Upgrading the program with a paused build — requires the timelock to pass.

This is by design. Do not add a pause.

## What's NOT in this guide

- Audit firm engagement (separate procurement)
- Squads multisig provisioning (Squads UI; document the threshold + signers internally)
- KYC/MSB compliance for the protocol entity (legal counsel)
- Insurance fund treasury management (treasury team)

## Devnet deploy log

First devnet deploy executed 2026-05-02 via Track A's documented Docker recipe. Build = current main (post-#30 typed reads + post-#32 audit fixes + post-#34 event-cpi feature flag).

| Program | Devnet program ID | Size | Authority | ProgramData address | Slot |
|---|---|---|---|---|---|
| credmesh_reputation | `JDBeDr9WFhepcz4C2JeGSsMN2KLW4C1aQdNLS2jvc79G` | 248,136 B | `6kWsEUqzLNaJgKbkstJUtYFWq56E1ZyYDeQ25XjChm7X` (deployer) | `AgVESbPqPLj6cA1HVobeiLYDZSnsTfv9Xzmj9EDxDxBi` | 459658199 |
| credmesh_receivable_oracle | `ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk` | 297,784 B | same | `GGT6vzAyPrkdRZMzSF4ixXBjdCm1EaUfdNZj8JEpyDUy` | 459658246 |
| credmesh_escrow | (program ID `DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF` reserved; not yet deployed — pending wallet top-up) | 467,464 B | — | — | — |

Verify: `solana program show <PROGRAM_ID> --url devnet`.

### Deploy cost (actual)
- reputation: 1.73 SOL
- receivable_oracle: 2.08 SOL
- escrow: ~3.26 SOL (estimated, not deployed)
- TOTAL needed for full first-deploy: ~7.07 SOL

Funded via faucet.solana.com (web faucet — `solana airdrop` rate-limits the public RPC pool tightly enough that programmatic airdrops cannot reliably acquire the >5 SOL needed in one burst).

### Remaining steps before mainnet
1. Deploy `credmesh_escrow` once wallet has ≥3.5 SOL (rent + buffer + tx fee margin).
2. Run `npm run init:oracle` with real governance + worker-authority pubkeys.
3. Run `npm run init:pool` with the asset_mint (devnet USDC `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`), governance, treasury_ata.
4. Verify init events on Solana Explorer.
5. Activate IDL flow (issue #15) so TS clients can construct typed instructions.
6. Rotate program-deploy keypairs + transfer upgrade authority to a Squads vault before any mainnet move.
