# TypeScript packages

```
ts/
├── shared/      @credmesh/solana-shared — TS mirror of Rust constants
│                (PDA seeds, ed25519 message offsets, attestor kinds) +
│                Anchor discriminator helpers. Imported by every other
│                ts/ package; the single TS-side mirror of
│                crates/credmesh-shared.
├── server/      Hono backend serving the public agent card
│                (/.well-known/agent.json) and SIWS nonce issuance.
├── bridge/      EVM ⇒ Solana attestation bridge. HTTP /quote signs the
│                canonical 128-byte ed25519_credit_message that
│                credmesh-escrow's request_advance consumes; Solana → EVM
│                event tail keeps EVM AgentRecord in sync. See
│                ts/bridge/README.md for env + trust model.
└── keeper/      Permissionless liquidation crank.
```

Each package is a self-contained `npm` workspace; install per-package
when you run it (`cd ts/<name> && npm install`).
