# DEPLOYMENT_PLAN.md

CredMesh-Solana v1 — devnet bring-up → mainnet readiness. Written
against `origin/main @ 6317555` (post-`evm-parity` squash merge).
Internal-only; lives under `internal/` (gitignored).

This plan is NOT a copy of HANDOFF.md. It is the path forward, with
the prerequisites, the rollback paths, and the open issues I found
that block a clean run today.

---

## 0. State of the repo (verified, not assumed)

Confirmed against `origin/main` and the actual handler sources:

| Claim from HANDOFF.md | Status |
|---|---|
| Two on-chain programs: `credmesh-escrow`, `credmesh-attestor-registry` | ✅ |
| `credmesh-reputation` and `credmesh-receivable-oracle` deleted/renamed | ✅ |
| 9 ixs on escrow / 4 ixs on registry | ✅ |
| ed25519 verification flow (sysvar introspection, signer-in-registry, kind, version, chain_id, agent/pool, freshness, expiry) | ✅ matches code |
| Per-agent rolling-window cap (`AGENT_WINDOW_SECONDS = 24h`, `agent_window_cap`) | ✅ |
| `ConsumedPayment` permanent / `init` not `init_if_needed` | ✅ |
| `LIQUIDATION_GRACE_SECONDS = 14d`, `CLAIM_WINDOW_SECONDS = 7d`, `PROTOCOL_FEE_BPS = 1500` | ✅ |
| Bridge: rate-limit, auth tokens, agent-binding map, refuses unknown agents | ✅ |
| Keeper: fixed-size Advance decode, shared blockhash per tick, `Promise.allSettled` | ✅ |

Discrepancies between HANDOFF.md and the code (worth knowing before mainnet):

1. **Settlement waterfall math.** HANDOFF.md §7.3 says
   `protocol_cut = principal × PROTOCOL_FEE_BPS / 10_000` (15% of
   principal). The actual handler
   (`programs/credmesh-escrow/src/instructions/claim_and_settle.rs`)
   computes `protocol_cut = (fee_owed + late_penalty) × 15% / 10000`
   — **15% of the fee**, not the principal. The economics are
   meaningfully different. Treat the code as canon and update the
   handoff before any external audit.
2. **Liquidation rent payout.** HANDOFF.md §7.4 says `Advance` closes
   on liquidate with `rent → cranker (MEV-neutral)`. The actual
   handler keeps `Advance` alive with `state = Liquidated` for the
   audit trail (AUDIT AM-7) and does **not** refund rent to the
   cranker. There is no economic incentive for a third party to crank.
   This means the keeper is effectively a protocol-run service, not
   a permissionless MEV market — operationalize accordingly (see §8).
3. **`MIN_ADVANCE_ATOMS = 1 USDC` floor.** Not flagged in the handoff.
   Smaller advances revert. Surface in the bridge `/quote` error path
   before mainnet.
4. **`CLAIM_WINDOW_SECONDS = 7 days` pre-settlement window.** The
   agent cannot `claim_and_settle` until `now >= expires_at - 7d`.
   Receivables with TTL < 7d would never reach a settlement window
   under the current handler. The bridge `/quote` should reject
   `ttl_seconds < CLAIM_WINDOW_SECONDS + epsilon`, or the handler's
   guard should be relaxed for short-TTL advances.
5. **`target/deploy/` keypair filename drift.** The committed file is
   `target/deploy/credmesh_receivable_oracle-keypair.json` (old name).
   The deploy script (`scripts/deploy.ts`) reads
   `target/deploy/credmesh_attestor_registry-keypair.json` (new name).
   `.gitignore` whitelists the new name. **`npm run deploy` fails on
   a fresh checkout** until this is reconciled. See §1 step 0.
6. **CI does not run tests.** `.github/workflows/build.yml` runs
   `cargo check --workspace --locked`. `cargo fmt`, `cargo clippy`,
   and the `ts/server` typecheck are present but
   `continue-on-error: true`. `npm test`, `npm run typecheck` for
   `ts/bridge | ts/keeper | ts/shared`, and any anchor build are not
   wired up. Treat green CI as "compile-only" — not a substitute for
   the test plan in TESTING_PLAN.md.

None of these are show-stoppers; they are facts the deploy plan has
to account for.

---

## 1. Pre-flight (one-time, before any devnet bring-up)

### 1.0 Reconcile the keypair filename drift (deploy blocker)

The repo ships with `credmesh_receivable_oracle-keypair.json` but the
deploy script and the `.gitignore` whitelist assume
`credmesh_attestor_registry-keypair.json`. Pick **one** of these:

- **(A) Rename the committed file.** Preserves the on-chain devnet
  program ID `ALVf6iyB...`. One-line commit:
  ```
  git mv target/deploy/credmesh_receivable_oracle-keypair.json \
         target/deploy/credmesh_attestor_registry-keypair.json
  ```
- **(B) Patch the deploy script.** If renaming the on-disk file is
  fraught, point `scripts/deploy.ts` at the old filename. Less clean
  but lower-blast-radius.

Also remove the stale `target/deploy/credmesh_reputation-keypair.json`
— the program is deleted, the keypair is dead weight, and committing
it leaks the deploy key for a program that no longer exists in the
workspace.

Verify after fix:
```
sha256sum target/deploy/credmesh_attestor_registry-keypair.json
solana-keygen pubkey target/deploy/credmesh_attestor_registry-keypair.json
# expected pubkey: ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk
```

### 1.1 Three principals (NOT three keys)

The pre-pivot "three-key topology" is gone. v1 has two on-chain trust
roots:

| Principal | Role | Storage |
|---|---|---|
| **Bridge ed25519 signer** | Signs 128-byte credit attestations | HSM/KMS in v1.5; encrypted JSON file in v1 |
| **Squads governance multisig** | Owns `Pool.governance` + `AttestorConfig.governance` | Squads v4 vault PDA |

Off-chain accessories that are NOT trust roots but still need to be
provisioned:

- Program upgrade authority (initially the deployer wallet; must be
  transferred to the Squads vault before mainnet — §7).
- Keeper wallet (low-balance hot wallet, ~0.1 SOL per quarter — pays
  liquidate tx fees with no rent refund — see §0 finding 2).
- Treasury USDC ATA (destination for protocol cut).

### 1.2 Toolchain (per CONTRIBUTING.md)

```bash
rustup default 1.79.0
sh -c "$(curl -sSfL https://release.anza.xyz/v1.18.26/install)"
cargo install --git https://github.com/coral-xyz/anchor avm --force && avm install 0.30.1

# Pin Anchor to 0.30.1
avm use 0.30.1
anchor --version  # expect: anchor-cli 0.30.1
solana --version  # expect: solana-cli 1.18.26
```

If `cargo install anchor avm` fails on Rust 1.79 (the `edition2024`
regression noted in `.github/workflows/build.yml`), use the Docker
recipe from the now-internal DEPLOYMENT.md history:
`backpackapp/build:v0.30.1 + cargo-build-sbf --tools-version v1.50`.

### 1.3 EVM-side prerequisites

The bridge reads two EVM contracts. They must be deployed and reachable
from the bridge host **before** any Solana pool accepts traffic:

| Contract | Method consumed by bridge | Notes |
|---|---|---|
| `ReputationCreditOracle` | `getCredit(address)` → `(score,totalExposure,maxExposure)` or legacy `maxExposure(address)` fallback | bridge uses `maxExposure` as the credit cap |
| `TrustlessEscrow` | `exposure(address)` → uint256 | EVM-lane outstanding only; Solana-local outstanding is added on-chain via `AgentIssuanceLedger.live_principal` |

Confirm with the EVM team that:
1. The exact contract addresses are set in env (`EVM_REPUTATION_CREDIT_ORACLE_ADDRESS`, `EVM_TRUSTLESS_ESCROW_ADDRESS`).
2. The ABI methods are stable (no upcoming breaking change).
3. There is an authenticated endpoint the EVM side replays into when
   the bridge POSTs to `EVM_CREDIT_WORKER_URL/solana-event`. Set
   `EVM_CREDIT_WORKER_TOKEN` in production so the event tail sends
   `Authorization: Bearer <token>`; otherwise accept that the event tail
   logs locally only, per the handoff fallback.

If the EVM contracts are not yet deployed to mainnet (Base), v1 is
**not** mainnet-ready regardless of Solana readiness.

---

## 2. Devnet bring-up (Cluster A)

### 2.1 Build artifacts

```bash
anchor build               # produces target/deploy/{escrow,attestor_registry}.so
ls -la target/deploy/      # confirm both .so files exist, sizes sane
sha256sum target/deploy/credmesh_escrow.so \
          target/deploy/credmesh_attestor_registry.so
```

Record the two SHAs. Every subsequent deploy must match these or fail
loud — the deploy script already does this check, but record the
expected SHAs in a deploy ticket so the verifier doesn't need to trust
the prompt.

### 2.2 Pool init keypair custody

`init_pool` in v1 is permissionless per `asset_mint` (seeds =
`[POOL_SEED, asset_mint]`). The first caller wins. **This is a
land-grab risk on mainnet.** On devnet you can re-run; on mainnet
you cannot.

Mitigation: pre-stage the `init_pool` transaction with the deployer's
keypair in the same atomic block (or the next-slot block) as the
program deploy. Do not pause between deploy and init.

### 2.3 Deployment sequence

Sign-off gate before starting: `npm run check`, `npm test`, and
`npm run typecheck` all green locally. CI will not catch a regression
in `npm test` or the four-package TS typecheck — do not skip.

```bash
# 1. Build (fresh).
anchor build

# 2. Deploy both programs. Deployer wallet is initial upgrade
#    authority — will be transferred to Squads in §7.
npm run deploy -- --cluster devnet \
  --wallet ~/.config/solana/id.json \
  --program all

# 3. Verify both program IDs are live.
solana program show --url devnet DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF
solana program show --url devnet ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk
```

Expected output: `Program Id`, `Owner: BPFLoaderUpgradeable...`,
`Authority: <deployer-pubkey>`, `Last Deployed In Slot: ...`.

### 2.4 Init the registry FIRST

The registry must exist before the escrow can derive `AllowedSigner`
PDAs in `request_advance`. The order is:

```bash
# Provision the Squads vault PDA address first (offline; via Squads UI).
# Save it as DEVNET_SQUADS_VAULT_PUBKEY in your env.

# Init the registry.
npm run init:registry -- --cluster devnet \
  --governance $DEVNET_SQUADS_VAULT_PUBKEY

# Verify.
solana account --url devnet $(npx ts-node scripts/derive_attestor_config.ts) \
  | head -10
```

`init_registry` accepts `--governance` and refuses `Pubkey::default()`.
Confirm the deployer wallet did NOT accidentally pass its own pubkey
as governance — the script has a guard but worth re-checking the
emitted `AttestorRegistryInitialized` event.

### 2.5 Init the pool

```bash
npm run init:pool -- --cluster devnet \
  --asset-mint 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU \
  --governance $DEVNET_SQUADS_VAULT_PUBKEY \
  --treasury-ata $DEVNET_TREASURY_USDC_ATA \
  --max-advance-pct-bps 3000 \
  --max-advance-abs 100000000 \
  --timelock-seconds 86400 \
  --chain-id 2 \
  --agent-window-cap 500000000
```

Decisions to record on the deploy ticket:
- `--max-advance-pct-bps`: 30% (devnet).
- `--max-advance-abs`: 100 USDC.
- `--timelock-seconds`: 86400 (24h). MUST match Squads timelock or
  governance ixs can fast-track parameter changes.
- `--chain-id`: 2 for devnet, 1 for mainnet. **Mismatch == handlers
  reject all attestations.**
- `--agent-window-cap`: 500 USDC/24h/agent. `0` disables — only
  acceptable for devnet bring-up. **Mainnet MUST be > 0.**

Verify the emitted `PoolInitialized` event:
```
solana logs --url devnet DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF
```

### 2.6 Whitelist the bridge ed25519 signer

```bash
# Generate the bridge's ed25519 keypair once. Store it offline.
solana-keygen new --no-bip39-passphrase \
  --outfile ~/keys/bridge-devnet.json
DEVNET_BRIDGE_PUBKEY=$(solana-keygen pubkey ~/keys/bridge-devnet.json)

# Build the add_allowed_signer payload (does NOT execute on its own —
# this prints a payload to drop into Squads vault_transaction_create).
npm run registry:add-signer -- --cluster devnet \
  --signer $DEVNET_BRIDGE_PUBKEY \
  --kind 0
```

Take the printed payload to the Squads UI/CLI:
1. `vault_transaction_create` with the printed inner ix.
2. Threshold approves.
3. `vault_transaction_execute` fires it on-chain.

Verify:
```
# AllowedSigner PDA derived from ALLOWED_SIGNER_SEED + bridge pubkey.
solana account --url devnet $(npx ts-node scripts/derive_allowed_signer.ts \
  --signer $DEVNET_BRIDGE_PUBKEY) | head -10
```

### 2.7 Bring up the bridge service

```bash
cd ts/bridge
npm install
# Write a binding map containing at least one (Solana pubkey, EVM
# address) pair for a smoke-test agent.
echo '{"<TEST_SOLANA_PUBKEY>": "0x<TEST_EVM_ADDRESS>"}' > bindings.devnet.json

SOLANA_RPC_URL=https://api.devnet.solana.com \
SOLANA_WS_URL=wss://api.devnet.solana.com \
EVM_RPC_URL=$BASE_SEPOLIA_RPC \
EVM_REPUTATION_CREDIT_ORACLE_ADDRESS=0x... \
EVM_TRUSTLESS_ESCROW_ADDRESS=0x... \
BRIDGE_SIGNING_KEY_PATH=~/keys/bridge-devnet.json \
BRIDGE_AGENT_BINDINGS_PATH=$(pwd)/bindings.devnet.json \
SOLANA_ESCROW_PROGRAM_ID=DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF \
SOLANA_ATTESTOR_REGISTRY_PROGRAM_ID=ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk \
SOLANA_CHAIN_ID=2 \
BRIDGE_AUTH_TOKENS=devnet-token-1,devnet-token-2 \
npm run dev
```

Health checks before letting any agent traffic in:
- `curl http://localhost:4001/.well-known/bridge` (or whatever health
  route is exposed) returns the signer pubkey and chain_id.
- A test `/quote` call with a valid binding returns a signed message
  and the EVM read works (non-zero `credit_limit_atoms` for the test
  agent).
- The `/quote` call with an UNKNOWN Solana pubkey returns an error
  (verifies the H-2 finding mitigation).

### 2.8 Bring up the keeper service

```bash
cd ts/keeper
npm install
RPC_URL=https://api.devnet.solana.com \
KEEPER_KEYPAIR_PATH=~/.config/solana/keeper.json \
ESCROW_PROGRAM_ID=DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF \
POOL_ASSET_MINT=4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU \
SCAN_INTERVAL_SECONDS=300 \
npm run dev
```

Fund the keeper wallet with ≥ 0.1 SOL on devnet. **The keeper is
out-of-pocket on every liquidation — there is no rent refund** (see
§0 finding 2). On mainnet, budget keeper SOL based on expected
liquidation frequency × 0.000005 SOL tx fee per liquidate.

### 2.9 First end-to-end advance

Block until this passes:

1. LP `deposit` 1000 USDC.
2. Bridge `/quote` for the test agent → returns signed attestation.
3. Agent submits `[ed25519_verify, request_advance(receivable_id, 10
   USDC, nonce)]`. Tx succeeds. `AdvanceIssued` event fires.
4. Bridge event tail logs `AdvanceIssued` (or POSTs authenticated
   decoded fields to `EVM_CREDIT_WORKER_URL/solana-event` if wired).
5. Agent `claim_and_settle(payment_amount = principal + fee_owed)`
   with a Memo ix carrying the `consumed.nonce`. Tx succeeds.
   `AdvanceSettled` event fires. Pool `total_assets` and
   `accrued_protocol_fees` increase by the expected amounts.
6. LP `withdraw(shares)` returns the original USDC + LP cut.

Failure modes to catch here:
- `ed25519` ix indexing mismatch (`Ed25519OffsetMismatch`).
- `chain_id` mismatch (`InvalidChainId`).
- `attested_at` clock skew between bridge and Solana cluster
  (`ReceivableStale` even with a fresh attestation — fix bridge
  NTP).
- Memo ix missing or carrying the wrong nonce
  (`MemoNonceMismatch`).

Repeat the loop ≥ 100 times against a small handful of test agents
before §3 (`V1_ACCEPTANCE` gate).

---

## 3. Devnet readiness gate

Before any mainnet promotion, the deploy ticket must check off:

- [ ] ≥ 100 advances issued + settled on devnet, no `Liquidated`
      tails from operational bugs.
- [ ] Bridge signer rotation rehearsed (§4). Old signer revoked.
      A previously-valid attestation from the old signer reverts
      with `Ed25519SignerUnknown`.
- [ ] Squads multisig is the **only** authority that can mutate
      `AttestorConfig.governance`, `Pool.governance`, `Pool.fee_curve`,
      and the `AllowedSigner` whitelist. Verify by attempting a
      governance-ix without the Squads CPI; expect
      `GovernanceRequired`.
- [ ] Pool params propose → timelock → execute cycle exercised once
      with a tiny fee-curve tweak. `ParamsProposed` and
      `ParamsExecuted` emit cleanly.
- [ ] Liquidation crank exercised end-to-end: a deliberately
      late-expiring advance fires, keeper picks it up, pool
      `total_assets` decreases by the lost principal, `AdvanceLiquidated`
      event fires, EVM-side reputation update lands (or is logged if
      EVM worker not wired).
- [ ] Bridge `clock-skew` monitoring in place — alert if
      `|bridge_now − cluster_now| > 30s` (handler tolerates 15min;
      30s is the early-warning).
- [ ] Treasury seeded with ≥ 5% of expected mainnet TVL (insurance
      buffer).
- [ ] `BRIDGE_AUTH_TOKENS` set in production env (`null` mode is
      acceptable on devnet only).

If any of the above is `[ ]`, mainnet is blocked.

---

## 4. Bridge signer rotation rehearsal (a v1 readiness gate)

Run this BEFORE the gate in §3 is signed off.

1. Generate a second bridge keypair: `solana-keygen new --outfile
   ~/keys/bridge-devnet-2.json`.
2. Build the `add_allowed_signer` payload for the new signer; route
   through Squads. Confirm `AllowedSignerAdded` event.
3. Bring up a second bridge instance with the new keypair on a
   distinct port. Confirm `/quote` returns the new signer's pubkey.
4. Cut a fraction of agent traffic to the new bridge. Confirm
   `request_advance` succeeds with the new signer.
5. Build the `remove_allowed_signer` payload for the OLD signer;
   route through Squads. Confirm `AllowedSignerRemoved` event.
6. Confirm that a pre-rotation attestation signed by the OLD signer
   that arrives during its 15-min TTL now reverts with
   `Ed25519SignerUnknown` (because the `AllowedSigner` PDA is closed
   and Anchor's seed verify fails).

**Rotation latency budget:** worst case 15 min until in-flight
attestations expire. Operationally, after `remove_allowed_signer`
lands, the old signer's pending attestations are immediately
unredeemable — there's no grace window.

---

## 5. Mainnet promotion sequence

Order matters. The Squads vault, the bridge, and the EVM-side
contracts must all be in their mainnet configuration **before** the
first mainnet deploy.

### 5.1 Provision Squads mainnet vault

Members, threshold, timelock. Recommended starting point:
- 3-of-5 multisig.
- 7-day timelock for `propose_params`.
- Two physical signers, one HSM signer, two software signers
  geographically separated.

Record the vault PDA. This is the `governance` value passed to every
init.

### 5.2 Generate fresh mainnet program keypairs

Devnet program IDs must NOT be reused on mainnet — they share an
authority history. Fresh keypairs only.

```bash
# Use anchor keys sync to regenerate after rm'ing the existing files.
rm target/deploy/credmesh_escrow-keypair.json
rm target/deploy/credmesh_attestor_registry-keypair.json
anchor keys sync   # generates fresh keypairs + updates declare_id!
                   # in both lib.rs files AND Anchor.toml's [programs.mainnet]
git diff           # confirm the two declare_id! macros and Anchor.toml entries
anchor build       # rebuild .so with the new program IDs
```

Record the new program IDs. Update HANDOFF.md, README.md, and
`scripts/lib/program-ids.ts` to reflect mainnet IDs in the same
commit.

### 5.3 Deploy + init on mainnet

```bash
# 1. Deploy (deployer wallet = initial upgrade authority).
npm run deploy -- --cluster mainnet \
  --wallet ~/keys/mainnet-deployer.json \
  --program all

# 2. Init registry with mainnet Squads vault.
npm run init:registry -- --cluster mainnet \
  --governance $MAINNET_SQUADS_VAULT_PUBKEY

# 3. Init pool (chain_id = 1, agent_window_cap MUST be > 0).
npm run init:pool -- --cluster mainnet \
  --asset-mint EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v \
  --governance $MAINNET_SQUADS_VAULT_PUBKEY \
  --treasury-ata $MAINNET_TREASURY_USDC_ATA \
  --max-advance-pct-bps <gov-decided> \
  --max-advance-abs <gov-decided> \
  --timelock-seconds 604800 \
  --chain-id 1 \
  --agent-window-cap <gov-decided>

# 4. Whitelist the mainnet bridge signer via Squads.
```

### 5.4 Bring up mainnet bridge + keeper

Same as §2.7/§2.8, but:
- HSM/KMS for the bridge signing key (if v1.5 hardening is on schedule;
  otherwise document the v1 file-on-disk limitation in the runbook).
- `EVM_RPC_URL` points at Base mainnet, not Base Sepolia.
- `SOLANA_CHAIN_ID=1`.
- `BRIDGE_AUTH_TOKENS` is mandatory.

### 5.5 Transfer program upgrade authority to Squads

**Critical** — without this, the deployer wallet can unilaterally
push a malicious upgrade.

```bash
# For each program.
solana program set-upgrade-authority \
  --url mainnet \
  --keypair ~/keys/mainnet-deployer.json \
  <PROGRAM_ID> \
  --new-upgrade-authority $MAINNET_SQUADS_VAULT_PUBKEY

# Verify.
solana program show --url mainnet <PROGRAM_ID>
# Authority: $MAINNET_SQUADS_VAULT_PUBKEY  ← expected
```

Once the Squads vault is the upgrade authority, future deploys
require:
1. `solana program write-buffer <new.so>` (returns a buffer pubkey).
2. `solana program set-buffer-authority <buffer> --new-buffer-authority $SQUADS_VAULT`.
3. Squads `vault_transaction_create` whose inner ix is
   `BPFLoaderUpgradeable::Upgrade { buffer, ... }`.
4. Multisig approves; executor fires.

### 5.6 Smoke-test on mainnet with a single small advance

- 1 USDC deposit, 1 USDC advance (== `MIN_ADVANCE_ATOMS`), settle.
- Confirm `total_assets`, `accrued_protocol_fees`, and the treasury
  ATA balance change as expected.
- If anything looks off, **freeze new advances at the bridge layer**
  (stop the `/quote` service) and investigate. The on-chain protocol
  has no `paused` flag (load-bearing invariant — AUDIT P0-6); the
  only pause point is upstream of the bridge.

---

## 6. Rollback paths

| Failure | Rollback |
|---|---|
| Program deploy succeeds but `init_pool` or `init_registry` reverts | Programs are idle; re-run after fixing arg. No on-chain state lost. |
| `init_pool` succeeds with wrong `chain_id` | Pool is dead-end (no attestation will verify). New pool requires new `asset_mint` (only USDC matters, so this is effectively bricked for that mint). **Mitigation:** v2-only escape hatch — deploy a new escrow program at a fresh program ID and migrate LPs by hand. There is no in-protocol migration. **Verify chain_id before broadcasting init_pool.** |
| Bridge key compromise detected | Squads `remove_allowed_signer` for the compromised pubkey. 15-min worst-case window for in-flight fraudulent attestations. New attestations rejected immediately. |
| Bad upgrade pushed via Squads | If Squads still controls the upgrade authority, multisig approves a follow-up Upgrade ix with the previous .so. Buffer-write + set-buffer-authority + Squads ix. ~30 min cycle. **Confirm a known-good prior .so is archived before every upgrade.** |
| EVM contract returns wrong values (oracle compromise) | Bridge stops `/quote` at the operator's discretion. Outstanding advances remain in flight; their on-chain state is unaffected. New attestations gated until EVM-side fix lands. |
| Keeper stops cranking | Liquidations stall but the protocol remains live. LP withdrawals capped at idle liquidity (`total_assets - deployed_amount`); they may be temporarily under-idled. Manual `liquidate` calls from any wallet unblock the queue. |

---

## 7. Out-of-scope for this plan (track separately)

- HSM/KMS migration for the bridge signing key (v1.5 hardening).
- Quorum bridge signers (today: any-valid-sig).
- Permissionless `claim_and_settle` (reverted in the pivot;
  re-introducing requires the payer-pre-auth pattern we punted on).
- Light Protocol compressed PDAs, Token-2022, MWA.
- Multi-asset pools (v1 is USDC-only by the single `asset_mint` per
  pool PDA invariant).
- Multi-issuer SAS attestations (v1.5; schema documented in the
  internal DESIGN).

---

## 8. Open issues to surface

These are findings from reading the code (verified, not asserted),
documented here so they end up in the deploy ticket and not lost.

1. **Keeper economics.** No rent payout on `liquidate` means no
   third-party MEV market. Operationalize the keeper as a
   protocol-run service. If we later want permissionless cranking,
   `liquidate` would need a `close = cranker` constraint on
   `Advance` — but that conflicts with the AUDIT AM-7 audit-trail
   rule. Discuss with the audit team before changing.
2. **Bridge clock-drift.** `request_advance` requires `attested_at
   <= now && (now - attested_at) <= 15 min`. Both sides are clock
   sensitive. Make sure the bridge host runs NTP and monitor
   `bridge_now − cluster_now`. Alert at 30s.
3. **`init_pool` permissionless creation.** A racer on mainnet could
   front-run the legitimate `init_pool` with their own `governance`
   and `treasury_ata` and claim the USDC pool slot. Mitigation:
   bundle the init_pool tx with the deploy tx via a Jito-style
   bundle, or use a low-publicity slot, or accept the risk (because
   the protocol's first-call wins, the LP wouldn't deposit into an
   adversarial pool because they can read the `governance` field).
4. **`MIN_ADVANCE_ATOMS = 1 USDC` and `CLAIM_WINDOW_SECONDS = 7d`.**
   Surface in the bridge `/quote` validation so the agent gets a
   clear 4xx before they pay tx fees.
5. **`Pool.max_advance_pct_bps` is currently unused by the handler.**
   The handler caps on `max_advance_abs`. The bps field is set in
   `init_pool` but not enforced by `request_advance` (only the abs
   cap is). Either wire it into the handler or drop the field — it's
   misleading as-is.
6. **CI gaps.** Wire up `npm test`, `npm run typecheck` (all four
   ts/ packages), `cargo clippy -- -D warnings` (real gate, not
   `continue-on-error`), and a Docker-based `anchor build` job
   before mainnet. Without these, regressions land silently.
7. **Stale `credmesh_reputation-keypair.json`.** Delete from the
   repo; the program is gone.

---

## 9. Post-deploy verification (mainnet, T+24h)

Block on each of these before declaring mainnet "live":

```
solana program show --url mainnet <ESCROW_ID>
# Authority: $MAINNET_SQUADS_VAULT_PUBKEY

solana program show --url mainnet <REGISTRY_ID>
# Authority: $MAINNET_SQUADS_VAULT_PUBKEY

# Pool sanity.
solana account --url mainnet <POOL_PDA> | head -20
# chain_id == 1, governance == squads, agent_window_cap > 0, fee_curve sane

# AttestorConfig sanity.
solana account --url mainnet <ATTESTOR_CONFIG_PDA> | head -10
# governance == squads

# At least one AllowedSigner present.
# AllowedSigner PDA derived from ALLOWED_SIGNER_SEED + bridge_pubkey
solana account --url mainnet <ALLOWED_SIGNER_PDA> | head -10
# kind == 0, signer == bridge pubkey

# Bridge health.
curl -H "Authorization: Bearer $TOKEN" https://bridge.mainnet/.well-known/bridge
# signer_pubkey_b58 == bridge pubkey, chain_id == 1

# First mainnet advance event has fired.
solana logs --url mainnet <ESCROW_ID> | grep AdvanceIssued | head -5

# Keeper is alive.
journalctl -u credmesh-keeper -n 50 | grep "no liquidatable advances\|liquidated"
```

If any of the above looks off, freeze the bridge before continuing.

---

## 10. Sign-off checklist

- [ ] Keypair filename drift reconciled (§1.0).
- [ ] Devnet ≥ 100 advances issued + settled, no operational bugs (§3).
- [ ] Bridge signer rotation rehearsed on devnet (§4).
- [ ] Mainnet Squads multisig configured, members confirmed, timelock set.
- [ ] Mainnet program keypairs freshly generated (§5.2).
- [ ] Program upgrade authority transferred to Squads vault (§5.5).
- [ ] EVM-side contracts deployed and reachable from bridge (§1.3).
- [ ] HSM/KMS path for bridge signing key documented (v1 acceptable;
      v1.5 plan written).
- [ ] Treasury seeded with insurance buffer (≥ 5% expected TVL).
- [ ] Bridge clock-drift monitoring alert in place.
- [ ] First mainnet advance + settle verified end-to-end (§5.6).
- [ ] Post-deploy verification at T+24h passes (§9).

If any box is unchecked, mainnet is not promoted.
