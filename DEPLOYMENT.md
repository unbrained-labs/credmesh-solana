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

```bash
solana config set --url devnet
solana airdrop 3
anchor build
anchor deploy --provider.cluster devnet
```

Note the deployed addresses; verify they match the keypair pubkeys.

### Initialize state on devnet

```bash
# 1. Init OracleConfig (worker authority + governance + caps)
ts-node scripts/init_oracle.ts \
  --cluster devnet \
  --governance <CredMesh devnet governance multisig vault PDA> \
  --worker-authority <hot worker key pubkey> \
  --worker-max-per-tx 100000000 \
  --worker-max-per-period 10000000000 \
  --period-seconds 86400

# 2. Init Pool (USDC, fee curve, caps, treasury ATA)
ts-node scripts/init_pool.ts \
  --cluster devnet \
  --asset-mint 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU \
  --governance <CredMesh devnet governance> \
  --treasury-ata <protocol treasury USDC ATA> \
  --max-advance-pct-bps 3000 \
  --max-advance-abs 100000000 \
  --timelock-seconds 86400
```

(Scripts not yet written; v1 sprint task.)

### Verify

- `solana program show <ESCROW_ID> --url devnet` — confirm deploy.
- `anchor verify <ESCROW_ID> --url devnet` — match local build hash.
- Run `npm test` — all Bankrun tests should pass on the now-deployed programs.

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
