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

1. **`DECISIONS.md`** — resolutions for the 5 blocking design questions (MPL vs SATI, Squads onboarding, Sybil mitigation, SAS roadmap, fee-payer). Start here.
2. **`AUDIT.md`** — three independent reviews of DESIGN + scaffold; 6 P0 fund-loss findings (all fixed mechanically) + the design questions DECISIONS resolves.
3. **`DESIGN.md`** — the v0 spec.
4. `research/CONTRARIAN.md` — why we're building it this way (vs literal EVM port).
5. `research/REVIEW.md` — what we got wrong in the first research pass.
6. `research/SYNTHESIS.md` — original mapping (superseded where they conflict).
7. `research/01–04` — supporting detail.

## Programs

| Program | Purpose | Status |
|---|---|---|
| `credmesh-shared` | Seed constants, program IDs, ed25519 message layout, `mpl_identity` + `cross_program` + `ix_introspection` helper modules | Implemented |
| `credmesh-escrow` | Pool vault + share-mint, advance issuance, settlement waterfall, governance | All v1 handlers implemented (not compile-verified) |
| `credmesh-reputation` | 8004-shape per-agent rolling-digest reputation; writer-gated EMA | Core handlers implemented; `append_response`/`revoke_feedback` stubbed |
| `credmesh-receivable-oracle` | Worker-attested + ed25519 payer-signed receivables, allowed-signer registry | All v1 handlers implemented |

External programs CredMesh **uses** but does not deploy: Squads v4 (agent vaults + governance), Solana Agent Registry (Metaplex Core asset), SPL Token, ed25519 native, Memo program.

## Building

```bash
# Install toolchains (Rust 1.79+, Solana 1.18.26, Anchor 0.30.1).
# See CONTRIBUTING.md for the exact commands.

anchor build
npm install
npm test           # ts-mocha + anchor-bankrun
```

Test layout follows DESIGN §7. Bankrun scaffold under `tests/bankrun/`:
- `escrow/init_pool.test.ts`, `escrow/deposit_withdraw.test.ts` — happy paths.
- `attacks/consumed_close_reinit.test.ts` — AUDIT P0-5 fixture.
- `attacks/ata_substitution.test.ts` — AUDIT P0-3 fixture.
- `attacks/sysvar_spoofing.test.ts` — AUDIT P1-2 fixture.
- `attacks/cross_agent_replay.test.ts` — asymmetric.re-class fix fixture.

Bodies are stubbed with the intended assertions in comments; they activate
once the IDL is generated.

## Deployment targets

- `devnet` — full-stack staging with Circle USDC faucet
- `mainnet-beta` — staged rollout with hard caps ($10–$100 advances)

Program IDs are placeholders (`CRED1escrow…`, `CRED1rep…`, `CRED1recv…`) and must be replaced with real keypair-derived IDs before deployment.
