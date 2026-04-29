# Bankrun tests

Fast in-process Solana runtime via [`anchor-bankrun`](https://github.com/kevinheavey/anchor-bankrun). 10x faster than `solana-test-validator`, ideal for unit/integration coverage of the three CredMesh programs.

## Layout (planned)

```
tests/bankrun/
├── escrow/
│   ├── init_pool.test.ts
│   ├── deposit_withdraw.test.ts
│   ├── request_advance_worker.test.ts
│   ├── request_advance_ed25519.test.ts
│   ├── claim_and_settle.test.ts
│   ├── liquidate.test.ts
│   ├── governance_timelock.test.ts
│   └── invariants.test.ts          # property-style: waterfall sum, share monotonicity
├── reputation/
│   ├── init_reputation.test.ts
│   └── give_feedback_writer_gating.test.ts   # DECISIONS Q4 single-writer test
├── receivable_oracle/
│   ├── worker_update.test.ts
│   └── ed25519_record.test.ts
└── attacks/
    ├── cross_agent_replay.test.ts             # AUDIT integration #2 fixture
    ├── consumed_close_reinit.test.ts          # AUDIT P0-5 fixture
    ├── ata_substitution.test.ts               # AUDIT P0-3 fixture
    └── sysvar_spoofing.test.ts                # AUDIT P1-2 fixture
```

## Required packages (added in v1 sprint)

```json
{
  "devDependencies": {
    "@coral-xyz/anchor": "^0.30.1",
    "@solana/web3.js": "^1.95.0",
    "@solana/spl-token": "^0.4.0",
    "anchor-bankrun": "^0.5.0",
    "solana-bankrun": "^0.4.0",
    "chai": "^5.0.0",
    "ts-mocha": "^10.0.0",
    "@types/chai": "^5.0.0",
    "@types/mocha": "^10.0.0"
  }
}
```

## Running

```bash
anchor build
anchor test --skip-local-validator
```

Anchor.toml's `[scripts] test = "yarn run ts-mocha ..."` invokes the bankrun runner.

## Status

- [ ] First test scaffolds land with the first handler implementation (init_pool happy path).
- [ ] Attack fixtures land alongside their corresponding fix to prove the fix holds.

## Why Bankrun (not solana-test-validator)?

`solana-test-validator` boots a full validator — slow, RPC-bound, hard to fixture. Bankrun runs the runtime in-process, lets you set clock/slot directly, and snapshots account state cheaply. The Drift, MarginFi, and Save teams all use it for the bulk of their integration coverage.
