# TESTING_PLAN.md

CredMesh-Solana v1 — layered test plan. Built against `origin/main @
6317555` (post-`evm-parity` squash merge). Internal-only; lives under
`internal/` (gitignored).

The premise of this plan: the code is "code-complete for v1" but the
test coverage is thin. Today there are **16 pure-math Rust unit tests
+ 2 program-id tests, no bankrun tests, no localnet integration tests,
no bridge contract tests, no Rust↔TS parity tests, no chaos tests**.
The pre-pivot bankrun fixtures were deleted in commit `d3b9649`.

For a credit protocol that holds USDC and disburses on an EVM-signed
attestation, this is below the bar. This plan brings it up.

---

## 0. Test pyramid (top-down, by cost)

| Layer | Where | Latency | Cost | Coverage |
|---|---|---|---|---|
| L1: Pure-math unit | `cargo test --lib` | ms | free | pricing, layout, AttestorKind |
| L2: Behavioural unit (bankrun) | `programs/credmesh-escrow/tests/` | seconds | free | every handler path, every revert |
| L3: Rust↔TS parity | new harness | seconds | free | pricing.rs ↔ pricing.ts, ed25519 layout |
| L4: Localnet integration | `anchor test` | minutes | free | end-to-end with cloned mainnet programs |
| L5: Devnet smoke | scripted | tens of minutes | low | with real bridge + keeper |
| L6: Chaos + rotation | scripted | hours | medium | clock skew, key rotation, partial outages |
| L7: Mainnet shadow | optional | days | high | tiny pool, real economics |

L1, L2, L3, L4 are CI-gateable and MUST be green before any merge to
main. L5, L6 are pre-promotion gates. L7 is optional but recommended
for the first week of mainnet.

---

## 1. Invariants → tests (the ledger)

A test that can't tell you which invariant it defends is a test that
will rot. Every test in this plan maps to a load-bearing invariant.

### 1.1 Cryptographic / attestation

| Invariant | Test ID | Layer |
|---|---|---|
| The prior ix MUST be the ed25519 native program | T-CRY-01 | L2 |
| The ed25519 ix MUST have `num_signatures == 1` | T-CRY-02 | L2 |
| All three offset-indices in the ed25519 ix MUST equal the ix's own index (asymmetric.re defense) | T-CRY-03 | L2 |
| `signed_pubkey == allowed_signer.signer` (registry lookup) | T-CRY-04 | L2 |
| `allowed_signer.kind == AttestorKind::CreditBridge` | T-CRY-05 | L2 |
| `signed_msg.len() == TOTAL_LEN (128)` | T-CRY-06 | L2 |
| `version == 1` | T-CRY-07 | L2 |
| `msg.agent_pubkey == ctx.accounts.agent.key()` | T-CRY-08 | L2 |
| `msg.pool_pubkey == ctx.accounts.pool.key()` | T-CRY-09 | L2 |
| `msg.nonce == nonce_arg` (binding to caller-supplied nonce) | T-CRY-10 | L2 |
| `msg.chain_id == pool.chain_id` (cross-cluster replay) | T-CRY-11 | L2 |
| `(now - attested_at) <= 15 min && attested_at <= now` | T-CRY-12 | L2 |
| `expires_at > now` | T-CRY-13 | L2 |
| `ConsumedPayment` is `init`, NOT `init_if_needed` (close-then-reinit defense) | T-CRY-14 | L2 |
| Same `(pool, agent, receivable_id)` cannot fire `request_advance` twice | T-CRY-15 | L2 |
| A `liquidate` does NOT close `ConsumedPayment` | T-CRY-16 | L2 |

### 1.2 Underwriting / issuance

| Invariant | Test ID | Layer |
|---|---|---|
| `amount <= attested_credit_limit - (attested_evm_outstanding + live_principal)`; no double-count when EVM has replayed Solana events | T-UND-01 | L2 |
| `amount <= pool.max_advance_abs` | T-UND-02 | L2 |
| `amount >= MIN_ADVANCE_ATOMS` | T-UND-03 | L2 |
| Per-agent window cap: `issued_in_window + amount <= pool.agent_window_cap` (when cap > 0) | T-UND-04 | L2 |
| Window reset after `AGENT_WINDOW_SECONDS` | T-UND-05 | L2 |
| `agent_window_cap == 0` disables the cap (devnet) | T-UND-06 | L2 |
| `pool.deployed_amount` increases by `amount` post-issue | T-UND-07 | L2 |
| `pool.deployed_amount <= pool.total_assets` (no over-deploy) | T-UND-08 | L2 |
| `Advance.attestor` is recorded (audit trail) | T-UND-09 | L2 |

### 1.3 Settlement waterfall

| Invariant | Test ID | Layer |
|---|---|---|
| `total_owed = principal + fee_owed + late_penalty` | T-SET-01 | L2 |
| `payment_amount >= total_owed` (revert otherwise) | T-SET-02 | L2 |
| `protocol_cut = (fee_owed + late_penalty) × PROTOCOL_FEE_BPS / BPS_DENOMINATOR` — **15% of fee, not principal** | T-SET-03 | L2, L3 |
| `lp_cut = principal + (total_fee - protocol_cut)` | T-SET-04 | L2 |
| `agent_net = payment_amount - protocol_cut - lp_cut` | T-SET-05 | L2 |
| Sum invariant: `protocol_cut + lp_cut + agent_net == payment_amount` | T-SET-06 | L2 |
| Memo ix MUST carry `consumed.nonce` | T-SET-07 | L2 |
| `now >= expires_at - CLAIM_WINDOW_SECONDS (7d)` (else `NotSettleable`) | T-SET-08 | L2 |
| Settlement window: receivable with TTL < 7d cannot be settled — **escalate this** (see DEPLOYMENT §0.4) | T-SET-09 | L2 |
| `late_days` clamps at `MAX_LATE_DAYS (365)` | T-SET-10 | L2 |
| `Advance.state` transitions Issued → Settled atomically | T-SET-11 | L2 |
| `Advance` is closed (rent → agent) on settle | T-SET-12 | L2 |
| Wrong-agent attempt at `claim_and_settle`: `InvalidPayer` | T-SET-13 | L2 |

### 1.4 Liquidation

| Invariant | Test ID | Layer |
|---|---|---|
| `now >= expires_at + LIQUIDATION_GRACE_SECONDS (14d)` (else `NotLiquidatable`) | T-LIQ-01 | L2 |
| `pool.deployed_amount -= principal` | T-LIQ-02 | L2 |
| `pool.total_assets -= principal` (LPs absorb the loss via share-price drop) | T-LIQ-03 | L2 |
| `Advance.state` transitions Issued → Liquidated (kept alive for audit trail, AUDIT AM-7) | T-LIQ-04 | L2 |
| `Advance` is NOT closed on liquidate (no rent → cranker) — **economic note, see DEPLOYMENT §0.2** | T-LIQ-05 | L2 |
| `ConsumedPayment` is NOT closed on liquidate | T-LIQ-06 | L2 |
| Cannot liquidate a Settled or already-Liquidated advance | T-LIQ-07 | L2 |
| `consumed.agent == advance.agent` constraint (AUDIT P0-1) | T-LIQ-08 | L2 |

### 1.5 Pool / share math

| Invariant | Test ID | Layer |
|---|---|---|
| First depositor inflation attack: 1-atom deposit costs ≥ 10⁶× extractable profit | T-POOL-01 | L1 |
| `preview_deposit` round-trip: `preview_redeem(preview_deposit(x)) <= x` (rounded down) | T-POOL-02 | L1 |
| `preview_deposit(0) == 0`, `preview_redeem(0) == 0` | T-POOL-03 | L1 |
| Withdraw caps at idle liquidity: `total_assets - deployed_amount` | T-POOL-04 | L2 |
| Deposit/withdraw don't change `total_assets - deployed_amount` invariant when no advances are in flight | T-POOL-05 | L2 |

### 1.6 Governance

| Invariant | Test ID | Layer |
|---|---|---|
| `propose_params` requires Squads CPI | T-GOV-01 | L2 |
| `execute_params` requires `now >= pending_params.execute_after` | T-GOV-02 | L2 |
| `pending_params` is cleared after `execute_params` | T-GOV-03 | L2 |
| `FeeCurve::validate()` rejects malformed curves (`kink_bps > 10_000`, `base > kink > max`, etc.) — at propose AND init | T-GOV-04 | L2 |
| `add_allowed_signer` / `remove_allowed_signer` / `set_governance` all require Squads CPI | T-GOV-05 | L2 |
| `governance != Pubkey::default()` enforced at init and at `set_governance` | T-GOV-06 | L2 |
| `Pool.max_advance_pct_bps` field — **currently unused by handler; either wire or drop. Flag the test as failing intentionally until this is decided.** | T-GOV-07 | L2 |

### 1.7 Cross-program reads

| Invariant | Test ID | Layer |
|---|---|---|
| `AllowedSigner` PDA owner verify: a fake account at the right address with wrong owner fails | T-CPR-01 | L2 |
| `AllowedSigner` discriminator verify: a wrong-discriminator account at the right (owner, address) fails | T-CPR-02 | L2 |
| Seed-derived `seeds::program` mismatch fails | T-CPR-03 | L2 |
| ATA substitution attack: passing a different agent's USDC ATA fails the `token::authority == agent` constraint | T-CPR-04 | L2 |
| Treasury ATA substitution: passing a non-`pool.treasury_ata` fails on the `address =` constraint | T-CPR-05 | L2 |

### 1.8 Bridge

| Invariant | Test ID | Layer |
|---|---|---|
| Unknown Solana pubkey → 4xx (no signed attestation produced) | T-BR-01 | L4-equivalent (vitest) |
| `nonce_hex` strictly 16 bytes (32 hex chars) | T-BR-02 | vitest |
| `ttl_seconds` clamped to `[1, 15 × 60]` | T-BR-03 | vitest |
| Output `expires_at - attested_at == ttl_seconds` | T-BR-04 | vitest |
| Output `chain_id` matches env `SOLANA_CHAIN_ID` | T-BR-05 | vitest |
| Output is exactly 128 bytes; layout matches Rust | T-BR-06 | vitest + L3 parity |
| EVM `creditLimit == 0` → 4xx (refuse to attest) | T-BR-07 | vitest |
| Token-bucket rate limit: burst + steady-state enforced per key | T-BR-08 | vitest |
| Missing `BRIDGE_AUTH_TOKENS` env in production mode → refuse to start | T-BR-09 | vitest |
| Agent-binding file empty → refuse to start | T-BR-10 | vitest |
| Agent-binding entry with malformed EVM address → refuse to load | T-BR-11 | vitest |

### 1.9 Keeper

| Invariant | Test ID | Layer |
|---|---|---|
| Advance decoder reads 152-byte fixed layout correctly | T-KEEP-01 | vitest |
| Filter: `state == Issued` only (Settled/Liquidated skipped) | T-KEEP-02 | vitest |
| Filter: `expires_at + grace <= now` only | T-KEEP-03 | vitest |
| Shared blockhash per tick (no N round-trips) | T-KEEP-04 | vitest, code review |
| `Promise.allSettled` semantics: one liquidate failure doesn't abort the batch | T-KEEP-05 | vitest |
| Liquidate ix accounts are in the order the handler expects | T-KEEP-06 | L4 |

### 1.10 Parity (Rust ↔ TypeScript)

| Invariant | Test ID | Layer |
|---|---|---|
| `compute_fee_amount` Rust output == TS output for 100 random inputs | T-PAR-01 | L3 |
| `preview_deposit` Rust output == TS off-chain quote for 100 random inputs | T-PAR-02 | L3 |
| `compute_late_penalty_per_day` parity | T-PAR-03 | L3 |
| 128-byte attestation: TS encoder output == Rust decoder expected input, byte-for-byte | T-PAR-04 | L3 |
| `AttestorKind` enum: `as_u8` round-trip is identical | T-PAR-05 | L3 |
| Anchor discriminators: TS computed == Rust computed for `Advance`, `Pool`, `ConsumedPayment`, `liquidate`, `request_advance` | T-PAR-06 | L3 |
| `MAX_ATTESTATION_AGE_SECONDS`, `CLAIM_WINDOW_SECONDS`, `LIQUIDATION_GRACE_SECONDS`, `AGENT_WINDOW_SECONDS`, `PROTOCOL_FEE_BPS`, `MIN_ADVANCE_ATOMS`, `MAX_LATE_DAYS` — TS mirror values == Rust values | T-PAR-07 | L3 |

That last one is cheap and catches the most insidious bug class.

---

## 2. L1 — Pure-math unit tests (Rust)

Already present: 16 tests in `programs/credmesh-escrow/src/pricing.rs`
under `#[cfg(test)]`. Run with `cargo test --workspace --lib`.

**Add:**

- T-POOL-02 round-trip property test (100 cases, `proptest` or
  hand-rolled). Skip if it pulls in `proptest` for one site — a
  loop with `for i in 0..100` and `rand::thread_rng()` is fine for
  v1.
- T-POOL-03 zero-input invariants.
- Pricing edge cases at the curve knee, at 100% utilization, at
  `default_count = 5` (clamp), at `max_late_days = 365` (clamp).

Estimated effort: 0.5 day. Coverage delta: pricing.rs from ~70% to
≥ 90%.

---

## 3. L2 — Behavioural unit tests (bankrun)

This is the largest gap. The pre-pivot bankrun stubs were deleted.
Recreate them properly.

### 3.1 Toolchain

Add to `package.json`:
```json
{
  "devDependencies": {
    "@coral-xyz/anchor": "0.30.1",
    "solana-bankrun": "^0.4.0",
    "anchor-bankrun": "^0.5.0",
    "ts-mocha": "^10.0.0",
    "chai": "^4.4.0",
    "@types/chai": "^4.3.0",
    "@types/mocha": "^10.0.0"
  },
  "scripts": {
    "test:bankrun": "ts-mocha -p tsconfig.json -t 60000 'programs/**/tests/**/*.test.ts'"
  }
}
```

Bankrun lets us deterministically warp the clock, which is critical
for the time-gated tests (T-SET-08, T-LIQ-01, T-CRY-12).

### 3.2 Test files

Organize by handler. One file per handler keeps the blast radius of
a state-setup change small.

```
programs/credmesh-escrow/tests/
├── helpers/
│   ├── setup.ts            (deploy program, init pool, mint USDC, helpers
│   │                       for building ed25519 ix + memo ix)
│   ├── ed25519.ts          (build a valid + invalid signed attestation)
│   └── memo.ts
├── init_pool.test.ts       (T-GOV-04, T-GOV-06 [delegate to registry tests])
├── deposit.test.ts         (T-POOL-04, T-POOL-05)
├── withdraw.test.ts        (T-POOL-04 idle-cap)
├── request_advance.test.ts (T-CRY-01..T-CRY-15, T-UND-01..T-UND-09, T-CPR-01..T-CPR-04)
├── claim_and_settle.test.ts (T-SET-01..T-SET-13, T-CPR-05)
├── liquidate.test.ts       (T-LIQ-01..T-LIQ-08, T-CRY-16)
├── propose_execute.test.ts (T-GOV-01..T-GOV-03)
└── skim_fees.test.ts       (smoke: accrued fees move from pool to treasury)

programs/credmesh-attestor-registry/tests/
├── init_registry.test.ts
├── add_remove_signer.test.ts (T-GOV-05, T-GOV-06)
└── set_governance.test.ts
```

### 3.3 Adversarial test fixtures (priority — these were on the
pre-pivot list and got dropped)

Each of these is a single bankrun test that mounts the attack and
asserts the handler rejects it.

1. **Cross-agent ed25519 replay** (T-CRY-08): agent A's signed
   attestation, replayed by agent B in their own `request_advance`.
   Expect `Ed25519MessageMismatch`.
2. **Sysvar instructions spoofing** (T-CRY-01): pass a fake
   instructions sysvar account (custom-deserializer trick). Expect
   the `#[account(address = sysvar_instructions::ID)]` to reject.
3. **`init_if_needed` close-then-reinit replay** (T-CRY-14): close
   `ConsumedPayment` (forge a tx that tries to), then re-init in
   the same tx. Expect Anchor's `init` to fail because the discriminator
   already exists.
4. **ATA substitution** (T-CPR-04): pass an attacker-controlled USDC
   ATA as `agent_usdc_ata`. Expect `token::authority == agent`
   constraint to reject.
5. **Treasury ATA substitution** (T-CPR-05): in `claim_and_settle`,
   pass an attacker ATA as `protocol_treasury_ata`. Expect the
   `address = pool.treasury_ata` constraint to reject.
6. **Memo nonce mismatch** (T-SET-07): tx omits the Memo or carries
   the wrong nonce bytes. Expect `MemoNonceMismatch`.
7. **Memo ix loop bound** (AUDIT MED #4): tx has 1000 inner ixs, none
   of which are the Memo. Expect no DoS — the introspection has a
   cap. Verify the cap is hit with a sane error, not a CU exhaustion.
8. **Chain ID cross-cluster replay** (T-CRY-11): attestation with
   `chain_id = 2` against a `chain_id = 1` pool. Expect
   `InvalidChainId`.
9. **Asymmetric.re relay** (T-CRY-03): craft an ed25519 ix where the
   offset-indices point at attacker-controlled bytes elsewhere in
   the tx. Expect `Ed25519OffsetMismatch`.
10. **Future-dated attestation** (T-CRY-12): `attested_at > now`.
    Expect `ReceivableStale`.
11. **Stale attestation** (T-CRY-12): `attested_at < now - 15min - 1s`.
    Expect `ReceivableStale`.

Estimated effort: 3 days for the full set with helpers.

### 3.4 Coverage gate

The bankrun suite should:
- Cover every revert path in every handler (`require!`, `require_eq!`,
  `require_keys_eq!`, every `constraint`).
- Cover the happy path of every handler.
- Cover at least one event-emission assertion per handler.

A loose target: `cargo-tarpaulin` or `cargo-llvm-cov` reports ≥ 85%
line coverage on `programs/credmesh-escrow/src/instructions/`. This
metric is approximate; the invariant ledger above is the real target.

---

## 4. L3 — Rust ↔ TypeScript parity

The handoff says "stays in lockstep" for `pricing.rs` ↔ `pricing.ts`,
the ed25519 message layout, and the constants in `ts/shared/`. There
is no test asserting this. Easy and high-value to add.

### 4.1 Approach

Compile `programs/credmesh-escrow/src/pricing.rs` to a `cdylib` (or
`wasm`) and link it into a vitest harness. Or simpler: write a small
Rust CLI under `tools/parity-oracle/` that takes JSON in, returns
JSON out, and the vitest harness shells out to it.

```
tools/parity-oracle/
├── Cargo.toml
└── src/main.rs   (reads JSON stdin, calls compute_fee_amount, prints JSON)
```

vitest fixtures:
```
ts/parity/__tests__/
├── fee.test.ts        (T-PAR-01, T-PAR-03)
├── shares.test.ts     (T-PAR-02)
├── attestation.test.ts (T-PAR-04)
├── constants.test.ts  (T-PAR-07)
└── discriminators.test.ts (T-PAR-06)
```

Property test driver: 100 random `(principal, duration_seconds,
utilization_bps, default_count, fee_curve)` tuples, generated with
a seeded PRNG so failures are reproducible.

Estimated effort: 1.5 days.

---

## 5. L4 — Localnet integration (Anchor test)

`anchor test` with cloned mainnet programs (USDC, Squads, Memo —
already configured in Anchor.toml's `[test.validator.clone]`).
Spins up a local validator with the deployed programs and lets us
exercise the full multi-tx flow.

### 5.1 Scenarios

1. **End-to-end happy path** (T-KEEP-06): LP deposit → bridge sign
   → `request_advance` → `claim_and_settle` → LP withdraw.
2. **End-to-end liquidation path**: same setup, advance to
   `expires_at + grace`, keeper picks up, `liquidate` lands, pool
   `total_assets` drops.
3. **Governance cycle**: Squads `vault_transaction_create` →
   threshold approve → `vault_transaction_execute` → `propose_params`
   → warp clock past timelock → `execute_params` → pool's fee curve
   updates.
4. **Multi-agent fanout**: 20 agents each pull a small advance
   simultaneously. Verify `agent_window_cap` enforces per-agent.
   Verify `pool.deployed_amount` aggregates correctly.
5. **First-deposit inflation attack** (against a live program, not
   just unit test): adversary deposits 1 atom, transfers 1M atoms
   directly to the vault, then a second LP deposits 1M atoms. Verify
   the second LP's share is fair (within the proof bounds).

### 5.2 Runner

```
tests/localnet/
├── e2e_happy_path.test.ts
├── e2e_liquidation.test.ts
├── governance_cycle.test.ts
├── multi_agent_fanout.test.ts
└── inflation_attack.test.ts
```

Estimated effort: 2 days.

---

## 6. L5 — Devnet smoke

After §5 passes locally, exercise the same scenarios on devnet with
the real bridge service running. Goal: catch issues that only manifest
at network latency, with real ed25519 program behaviour, with real
Squads vault execution.

Per DEPLOYMENT_PLAN.md §2.9, repeat the end-to-end loop ≥ 100 times
across ≥ 3 test agents. Spread the loop over ≥ 24 hours to exercise:
- Clock drift between bridge host and Solana cluster.
- RPC node failover.
- Bridge process restarts (in-flight quote should not become valid
  after restart — TTL-bound).

Telemetry to record:
- `request_advance` tx success rate.
- Average `bridge_signing_latency` (quote endpoint to client).
- `EVM_read` latency (separately, to isolate the EVM RPC cost).
- Keeper tick latency, advances-per-tick, liquidations-per-day.

If failure rate > 1% over 100 advances, devnet gate fails.

---

## 7. L6 — Chaos + rotation rehearsals

Block on these before mainnet.

| Rehearsal | Method | Expected outcome |
|---|---|---|
| Bridge signer rotation (§4 of DEPLOYMENT_PLAN.md) | Add new signer via Squads; cut traffic; remove old via Squads | Old signer attestations rejected with `Ed25519SignerUnknown` within seconds of `remove_allowed_signer` landing |
| Bridge clock skew | Run bridge with `date -s` offset of +30s, -30s, +14min, +16min | +30s/-30s: succeeds. +14min: succeeds. +16min: handler rejects `ReceivableStale` |
| RPC node outage | Block bridge from primary RPC; verify failover | Bridge serves cached EVM reads (if implemented; otherwise fails closed — acceptable in v1) |
| Squads vault key loss simulation | One member loses their key | 3-of-5: protocol still operable. 1-of-1: bricked (which is why 1-of-1 is not acceptable) |
| Keeper outage | Kill keeper for 24h | Advances accumulate in liquidatable state. Restart keeper → backlog drains. LP withdrawals limited to idle liquidity during the outage |
| Upgrade rehearsal | Push a no-op upgrade through Squads | Upgrade ix executes; program hash changes; programs continue functioning |
| Downgrade rehearsal | Push the previous .so as the new buffer | Same as above — confirms we can roll back via Squads. **Archive each prior .so before upgrading.** |

Estimated effort: 1.5 days (most of it scripting + waiting).

---

## 8. L7 — Mainnet shadow (optional)

Open a tiny pool on mainnet (1000 USDC seeded by the protocol team)
with a 1-of-1 internal allowlist for a week before public launch.
Run real advances + settlements with controlled agents. Verify:
- Treasury accrual matches the off-chain quote.
- No surprise CU exhaustion at mainnet's congestion levels.
- Bridge `/quote` p99 latency under load.

Not a substitute for §5, §6 — just a final reality check.

---

## 9. CI wiring

The current CI (`.github/workflows/build.yml`) runs:
- `cargo check --workspace --locked` (gating).
- `cargo fmt --check` (`continue-on-error`).
- `cargo clippy -- -D warnings` (`continue-on-error`).
- `ts/server` typecheck only (`continue-on-error`).

**Gaps that must close before mainnet:**

```yaml
# Gate everything.
- run: cargo fmt --all -- --check
- run: cargo clippy --workspace -- -D warnings
- run: cargo test --workspace --lib              # L1 pricing
- run: npm test                                  # alias for the above
- run: npm run test:bankrun                      # L2
- run: npm run typecheck                         # all four ts/ packages
- run: cd ts/bridge && npm test                  # vitest
- run: cd ts/keeper && npm test                  # vitest
- run: cd tools/parity-oracle && cargo build --release && cd ../.. && npm run test:parity   # L3
```

Plus a Docker-based anchor-build job (the previous one was removed
in PR #55 because of an `edition2024` regression — replacement is a
TODO, currently tracked informally per the workflow comment).

Without these in CI, every PR is a manual-verification game.

---

## 10. Test data + secrets

- **No real EVM addresses** in repo. Use synthetic 0x-pad addresses
  in fixtures.
- **No real bridge keypairs** in test fixtures. Generate per-test
  ephemeral keypairs.
- **USDC mint** in fixtures is a freshly-minted SPL token, not the
  real USDC mint. (For localnet, the cloned real USDC mint is fine
  but the test must not assume any pre-existing balance.)
- **Squads vault** in fixtures is a Pubkey value (the handler only
  reads the value, the CPI machinery is faked in helpers).

---

## 11. Out-of-scope (acknowledged gaps)

- **Fuzzing.** A `cargo fuzz` target for `pricing::compute_fee_amount`
  and `pricing::preview_deposit` is high-value but explicitly v1.5.
- **Formal verification.** Worth considering for the waterfall math
  given the sum invariant — but v1.5+.
- **Mutation testing.** `cargo-mutants` against the handlers. v1.5.
- **Performance benchmarks.** CU budgets per ix. v1.5.
- **Multi-pool / multi-asset.** Out of scope by DESIGN.

---

## 12. Effort estimate

| Layer | Effort | Owner |
|---|---|---|
| L1 add'l tests | 0.5d | rust |
| L2 bankrun suite (handlers) | 3d | rust+ts |
| L2 bankrun suite (adversarial fixtures) | 2d | rust+ts |
| L3 parity oracle | 1.5d | rust+ts |
| L4 localnet integration | 2d | ts |
| L5 devnet exercise | 1d (calendar; mostly waiting) | ts |
| L6 chaos + rotation | 1.5d | devops |
| CI wiring | 0.5d | devops |
| **Total** | **12d** | mixed |

That's a non-trivial chunk relative to "code-complete for v1." It's
also the gap between "code that probably works" and "code that we
can audit honestly." Don't compress.

---

## 13. Pass criteria for v1

- [ ] L1, L2, L3 fully green on main; CI gates them all.
- [ ] L4 green locally (cannot be gated in CI without docker).
- [ ] L5 devnet exercise complete (≥ 100 advances issued + settled,
      no operational bugs, instrumentation in place).
- [ ] L6 chaos rehearsals complete; runbooks recorded.
- [ ] Independent reviewer (not the original code author) re-runs L1,
      L2, L4 from a fresh checkout and confirms green.
- [ ] HANDOFF.md updated to reflect the two discrepancies in
      DEPLOYMENT_PLAN.md §0.
