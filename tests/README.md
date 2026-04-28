# Tests

Three layers (per `DESIGN.md` §7):

```
tests/
├── bankrun/    Fast unit/integration via anchor-bankrun
├── litesvm/    Property/fuzz via litesvm + proptest
└── devnet/     End-to-end with real Circle USDC + Squads + Helius webhooks
```

Pre-implementation. Test scaffolding lands with the first instruction implementation in `programs/credmesh-escrow/src/lib.rs`.
