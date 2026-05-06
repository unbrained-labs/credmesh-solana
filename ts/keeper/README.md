# credmesh-solana-keeper

Permissionless keeper service that scans the chain for expired
`Advance` PDAs and submits `liquidate()` ixs after the 14-day grace
period (`expires_at + LIQUIDATION_GRACE_SECONDS`).

Mirrors the EVM lane's `scripts/keeper-liquidate.ts`. Anyone can run
this — `liquidate` is permissionless on-chain (anchor account-struct
just requires `cranker: Signer`, not a specific authority). LP capital
is the principal beneficiary: liquidation flips an unrecovered advance
from `state == Issued` to `state == Liquidated` and the pool's
`deployed_amount` and `total_assets` both decrement by `principal`,
realising the LP loss via share-price drop.

## Env

| Var | Required | Default | Notes |
|---|---|---|---|
| `RPC_URL` | yes | — | Solana RPC endpoint (devnet / mainnet) |
| `KEEPER_KEYPAIR_PATH` | yes | — | Filesystem path to the keeper signer keypair (for tx signing) |
| `ESCROW_PROGRAM_ID` | yes | — | Escrow program ID (devnet `DLy82HRr...`) |
| `POOL_ASSET_MINT` | yes | — | Pool's USDC mint (devnet `4zMMC9srt...`) |
| `SCAN_INTERVAL_SECONDS` | no | `300` | How often to rescan (default 5 min) |
| `LIQUIDATION_GRACE_SECONDS` | no | `1209600` | 14 days. Match `state.rs::LIQUIDATION_GRACE_SECONDS` |

## How it works

1. Derive the Pool PDA: `[POOL_SEED, asset_mint]` under `ESCROW_PROGRAM_ID`.
2. `getProgramAccounts` filtered by Advance discriminator + `state == Issued`.
3. For each Advance, decode `expires_at` from the data buffer.
4. If `now >= expires_at + LIQUIDATION_GRACE_SECONDS`, build a
   `liquidate` ix and submit it.
5. Loop with `SCAN_INTERVAL_SECONDS` cadence.

## Status

Issue #15 (Anchor 0.30 IDL extraction) gates the typed Codama-generated
client. Until that lands, the keeper uses hand-rolled borsh decoding +
manually-constructed instruction data. This is intentionally minimal
to keep the surface auditable.

The keeper does **not** bundle `claim_and_settle` cranking — that's
the cranker's job, and Mode B / Mode 3 dispatch is handled in
`request_advance`-granted SPL delegates or marketplace-funded
settlements respectively. The keeper's only on-chain action is
`liquidate`.

## Running

```bash
npm install
RPC_URL=https://api.devnet.solana.com \
  KEEPER_KEYPAIR_PATH=~/.config/solana/keeper.json \
  ESCROW_PROGRAM_ID=DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF \
  POOL_ASSET_MINT=4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU \
  npm run dev
```
