# credmesh-solana

Research, design, and pre-implementation Anchor scaffolding for porting [CredMesh](https://github.com/unbrained-labs/credmesh) — a programmable credit protocol for autonomous agents — from EVM (Base) to Solana.

## Status

**Pre-implementation.** Anchor workspace is scaffolded — instruction signatures, account structs, error codes, and events are in place. Instruction bodies are stubbed (`Ok(())`). The EVM protocol is live at https://credmesh.xyz.

## Layout

```
credmesh-solana/
├── DESIGN.md                          implementer spec (v0)
├── Anchor.toml                        anchor workspace config
├── Cargo.toml                         workspace root
├── programs/
│   ├── credmesh-escrow/               vault + advance + claim_and_settle
│   ├── credmesh-reputation/           8004-shape, CredMesh-owned
│   └── credmesh-receivable-oracle/    worker-attested + ed25519-verified
├── ts/                                server, dashboard, mcp-server (pre-impl)
├── tests/                             bankrun / litesvm / devnet (pre-impl)
└── research/
    ├── 01-vault-escrow.md             on-chain vault, escrow, oracles
    ├── 02-identity-reputation.md      Solana Agent Registry / 8004-solana
    ├── 03-offchain-infra.md           SIWS, Helius Sender, Phantom Connect
    ├── 04-payments-oracles.md         x402 native, CCTP v2, credit pipeline
    ├── SYNTHESIS.md                   end-to-end architecture mapping
    ├── REVIEW.md                      critical pass on SYNTHESIS
    └── CONTRARIAN.md                  Solana-native redesign opportunities
```

## Read order

1. **`AUDIT.md`** — three independent reviews of DESIGN + scaffold; 6 P0 fund-loss findings, 8 open design questions. Read this first.
2. **`DESIGN.md`** — the v0 spec.
3. `research/CONTRARIAN.md` — why we're building it this way (vs literal EVM port).
4. `research/REVIEW.md` — what we got wrong in the first research pass.
5. `research/SYNTHESIS.md` — original mapping (superseded where they conflict).
6. `research/01–04` — supporting detail.

## Programs

| Program | Purpose | Status |
|---|---|---|
| `credmesh-shared` | Shared seed constants, program IDs, ed25519 message layout | Scaffold |
| `credmesh-escrow` | Pool vault + share-mint, `request_advance`, `claim_and_settle`, governance | Scaffold + audit fixes applied |
| `credmesh-reputation` | 8004-shape per-agent rolling-digest reputation | Scaffold + audit fixes applied |
| `credmesh-receivable-oracle` | Worker-attested + ed25519 payer-signed receivables, allowed-signer registry | Scaffold + audit fixes applied |

External programs CredMesh **uses** but does not deploy: Squads v4 (agent vaults + governance), Solana Agent Registry (Metaplex Core asset), SPL Token, ed25519 native, Memo program.

## Building

```bash
# (once Anchor toolchain is installed)
anchor build
```

Tests will run against `anchor-bankrun` (unit/integration) and `litesvm` (fuzz) per `DESIGN.md` §7.

## Deployment targets

- `devnet` — full-stack staging with Circle USDC faucet
- `mainnet-beta` — staged rollout with hard caps ($10–$100 advances)

Program IDs are placeholders (`CRED1escrow…`, `CRED1rep…`, `CRED1recv…`) and must be replaced with real keypair-derived IDs before deployment.
