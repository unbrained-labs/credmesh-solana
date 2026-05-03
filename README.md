# credmesh-solana

Anchor workspace porting [CredMesh](https://github.com/unbrained-labs/credmesh) — a programmable credit protocol for autonomous agents — from EVM (Base) to Solana.

## Status

**v1 implemented, audited, partial devnet deploy.** All 4 program crates have their v1 handlers landed (escrow, reputation, receivable-oracle, plus the credmesh-shared library). Compile-verified via Docker (see `DEPLOYMENT.md` § Build environment). Two of the three deployable programs are live on devnet:

| Program | Status | Devnet program ID |
|---|---|---|
| `credmesh-reputation` | ✅ deployed | `JDBeDr9WFhepcz4C2JeGSsMN2KLW4C1aQdNLS2jvc79G` |
| `credmesh-receivable-oracle` | ✅ deployed | `ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk` |
| `credmesh-escrow` | ⏳ keypair reserved, deploy pending wallet top-up | `DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF` |

Both deployed binaries verified byte-for-byte against local builds (SHA256 match). See `DEPLOYMENT.md § Devnet deploy log` for slots, ProgramData addresses, and authority. The EVM protocol is live at https://credmesh.xyz.

## Read order

1. **`docs/ARCHITECTURE.md`** — program structure, PDAs, cross-program edges (Mermaid diagrams).
2. **`docs/LOGIC_FLOW.md`** — sequence diagrams for every canonical handler + invariant table.
3. **`DECISIONS.md`** — resolutions for the 5 blocking design questions (MPL vs SATI, Squads onboarding, Sybil mitigation, SAS roadmap, fee-payer).
4. **`AUDIT.md`** — three independent reviews of DESIGN + scaffold; all P0/P1 findings fixed; post-EPIC postscript covers the 5-pass audit + audit-driven fixes.
5. **`DESIGN.md`** — the implementer spec.
6. **`DEPLOYMENT.md`** — Docker build recipe, deploy procedure, key rotation, devnet log.
7. **`V1_ACCEPTANCE.md`** — the gating checklist.
8. `research/CONTRARIAN.md` / `research/REVIEW.md` / `research/SYNTHESIS.md` / `research/01–04` — supporting research (some superseded).

## Layout

```
credmesh-solana/
├── docs/
│   ├── ARCHITECTURE.md            program graph + PDAs (Mermaid)
│   └── LOGIC_FLOW.md              per-handler sequence diagrams
├── crates/
│   └── credmesh-shared/           seeds, program_ids, cross_program helpers,
│                                  ix_introspection, ed25519_message layout
│                                  (library only — never deployed)
├── programs/
│   ├── credmesh-escrow/           vault + advance + claim_and_settle + liquidate
│   ├── credmesh-reputation/       8004-shape rolling digest, writer-gated EMA
│   └── credmesh-receivable-oracle/ worker + ed25519 payer-signed receivables
├── ts/server/                     Hono backend (SIWS auth, tx-builder, webhook ingress)
├── scripts/                       deploy.ts + init_oracle.ts + init_pool.ts
├── tests/bankrun/                 pure-math + scaffolded harness suites
├── target/deploy/                 committed devnet program keypairs
└── research/                      original research artifacts (some superseded)
```

## Programs

| Program | Purpose | Status |
|---|---|---|
| `credmesh-shared` (lib) | Seed constants, program IDs, ed25519 message layout, `mpl_identity` + `cross_program` + `ix_introspection` helper modules. Lives in `crates/`, not `programs/` (never deployed). | Implemented + compiled. |
| `credmesh-escrow` | Pool vault + share-mint, advance issuance, settlement waterfall, liquidate, governance. | All v1 handlers implemented + compiled. Deploy pending. |
| `credmesh-reputation` | 8004-shape per-agent rolling-digest reputation; writer-gated EMA via `emit_cpi!` for log-truncation defense. | Implemented + compiled + **deployed to devnet**. `append_response` / `revoke_feedback` are v1.5 stubs. |
| `credmesh-receivable-oracle` | Worker-attested + ed25519 payer-signed receivables, allowed-signer registry, source_kind-namespaced PDAs. | Implemented + compiled + **deployed to devnet**. |

External programs CredMesh **uses** but does not deploy: Squads v4 (agent vaults + governance), MPL Agent Registry + Agent Tools + Core (agent identity, executive profile), SPL Token, ed25519 native, Memo program.

## Building

The pinned Anchor 0.30.1 + Solana 1.18.26 toolchain has lockfile-drift issues against modern Cargo registry contents. The verified build recipe is in `DEPLOYMENT.md § Build environment (Docker)`. TL;DR:

```bash
# Pre-warm cached docker volumes (one-time):
docker pull backpackapp/build:v0.30.1
docker volume create credmesh-rustup
docker volume create credmesh-cargo-registry
docker volume create credmesh-cargo-git
docker volume create credmesh-pt-cache
docker run --rm -v credmesh-rustup:/root/.rustup backpackapp/build:v0.30.1 \
  rustup toolchain install 1.86.0 --profile minimal --no-self-update

# Then `anchor build --no-idl` via the wrapped invocation in DEPLOYMENT.md.
npm install
npm test           # ts-mocha + anchor-bankrun (pure-math suites run today; harness suites pending IDL fix)
```

`--no-idl` is a workaround until issue #15 (IDL extraction E0433 on `AssociatedToken`) lands. The deployable artifact is the `.so`, which `--no-idl` produces correctly.

## Tests

`tests/bankrun/` ships two layers:

- **Pure-math suites** that run today (waterfall sum invariant, share-price monotonicity, first-depositor inflation defense; ~2100 fuzz cases). 11/11 pass on current main.
- **Harness scaffolds** for behavioral tests (init_pool, deposit/withdraw, request_advance Worker + ed25519, claim_and_settle, liquidate, attack fixtures). Activate once the IDL gap closes.

## Deployment

`devnet` deploy is partial (see Status table above). `mainnet-beta` rollout requires:
1. Rotate program keypairs (devnet keys committed to repo are NOT for mainnet).
2. Transfer upgrade authority to a Squads vault per `DESIGN §10`.
3. Stage with hard caps ($10–$100 advances) per `DEPLOYMENT.md`.

See `DEPLOYMENT.md` for the full procedure.
