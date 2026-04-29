# DEPLOYMENT

How to deploy CredMesh Solana to devnet and mainnet-beta. **All steps assume v1 acceptance criteria are met (see [V1_ACCEPTANCE.md](./V1_ACCEPTANCE.md)).**

## Prerequisites

- Solana CLI 1.18.26+
- Anchor 0.30.1
- Rust 1.79.0+
- A funded deployer wallet (~3 SOL devnet, ~10 SOL mainnet for one-time costs)
- A pre-created Squads v4 governance multisig (mainnet only)

## Phase 0 — Generate program IDs

The placeholder IDs in source (`CRED1escrow…`, `CRED1rep…`, `CRED1recv…`, `CRED1shared…`) must be replaced with deterministic deploy keypairs.

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

Update **all four** source files with the new pubkeys:
- `programs/credmesh-escrow/src/lib.rs` — `declare_id!`
- `programs/credmesh-reputation/src/lib.rs` — `declare_id!`
- `programs/credmesh-receivable-oracle/src/lib.rs` — `declare_id!`
- `programs/credmesh-shared/src/lib.rs` — `program_ids::ESCROW`, `REPUTATION`, `RECEIVABLE_ORACLE`
- `Anchor.toml` — `[programs.devnet]` / `[programs.mainnet]` blocks

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
