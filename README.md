# credmesh-solana

The Solana lane of [CredMesh](https://github.com/unbrained-labs/credmesh)
— a programmable credit protocol for autonomous agents.

The EVM lane (live at https://credmesh.xyz) holds the canonical
agent identity, multi-attestor reputation, and timelocked governance.
**This Solana lane is a credit-issuance + settlement venue** that
consumes EVM-attested credit limits via short-TTL ed25519 signatures
from a whitelisted bridge, and runs the LP vault + advance + waterfall
on Solana rails.

## One-line flow

```
LP deposits USDC → agent gets a 15-min ed25519 attestation of its EVM credit
 → submits [ed25519_verify(...), request_advance(...)] on Solana
 → escrow disburses USDC against the attested limit
 → agent's job pays out → claim_and_settle(payment_amount) runs the
   3-tranche waterfall (protocol cut, LP cut, agent net)
 → if 14 days post-expiry the agent never settles, anyone runs liquidate
```

## Workspace

```
crates/
└── credmesh-shared/                Library — seeds, program IDs, ed25519
                                    message layout, AttestorKind enum,
                                    cross-program 4-step verify,
                                    instruction-sysvar introspection.
                                    NEVER deployed.
programs/
├── credmesh-escrow/                Pool vault + share-mint, advance,
│                                   3-tranche settlement, liquidation,
│                                   timelocked governance, virtual-shares
│                                   ERC-4626 math, per-agent rolling-window
│                                   issuance cap.
└── credmesh-attestor-registry/     Governance-controlled whitelist of
                                    bridge ed25519 signers (kind-tagged
                                    AllowedSigner PDAs).
ts/
├── shared/                         @credmesh/solana-shared — TS mirror of
│                                   Rust constants + Anchor discriminator
│                                   helpers.
├── bridge/                         EVM ⇒ Solana attestation bridge:
│                                   HTTP /quote signs the canonical 128-byte
│                                   ed25519_credit_message + Solana → EVM
│                                   event tail keeps EVM AgentRecord in sync.
├── server/                         Hono backend — agent card + SIWS nonce.
└── keeper/                         Permissionless liquidation crank.
scripts/                            Operator scripts (deploy, init_pool,
                                    init_registry, add_allowed_signer).
```

## Programs (devnet)

| Program | Devnet program ID |
|---|---|
| `credmesh-escrow` | `DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF` |
| `credmesh-attestor-registry` | `ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk` |

External programs CredMesh **uses** but does not deploy: Squads v4
(governance multisig), SPL Token classic (USDC vault + share mint),
the ed25519 native precompile, the Memo program (settlement nonce
binding).

## Build / test

```bash
# Toolchain (see CONTRIBUTING.md for full setup).
rustup default 1.79.0
sh -c "$(curl -sSfL https://release.anza.xyz/v1.18.26/install)"
cargo install --git https://github.com/coral-xyz/anchor avm --force && avm install 0.30.1

npm run check       # cargo check --workspace --locked
npm test            # cargo test --workspace --lib (16 pure-math + 2 program-id tests)
npm run typecheck   # ts/{shared,server,bridge,keeper}
npm run build       # anchor build
```

## Deploy (devnet)

```bash
anchor build
npm run deploy -- --cluster devnet --wallet ~/.config/solana/id.json --program all
npm run init:registry -- --cluster devnet --governance <SQUADS_VAULT_PUBKEY>
npm run init:pool     -- --cluster devnet \
  --asset-mint 4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU \
  --governance <SQUADS_VAULT_PUBKEY> \
  --treasury-ata <TREASURY_USDC_ATA> \
  --max-advance-pct-bps 3000 --max-advance-abs 100000000 \
  --timelock-seconds 86400 --chain-id 2 --agent-window-cap 500000000
npm run registry:add-signer -- --cluster devnet \
  --signer <BRIDGE_ED25519_PUBKEY> --kind 0
# Take the printed payload to the Squads UI; multisig approves; executor fires it.
```

## Run off-chain

```bash
# Bridge — see ts/bridge/README.md for the full env table.
cd ts/bridge && npm install && npm run dev

# Keeper.
cd ts/keeper && npm install && npm run dev

# Server (agent card + SIWS).
cd ts/server && npm install && npm run dev
```

## Documentation

- [`CLAUDE.md`](./CLAUDE.md) — repo conventions for contributors.
- [`CONTRIBUTING.md`](./CONTRIBUTING.md) — toolchain + workflow.
- [`ts/bridge/README.md`](./ts/bridge/README.md) — bridge env, trust model.

## License

(Pre-release; license TBD.)
