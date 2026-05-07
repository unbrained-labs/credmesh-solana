# ts/server

Hono backend for CredMesh Solana. Two responsibilities post-pivot:

1. Serve the public agent card at `GET /.well-known/agent.json` (cached
   at module load).
2. Issue SIWS (Sign-In With Solana, CAIP-122) nonces at
   `POST /auth/nonce`.

Everything credit-related (issuing advances, reading EVM state, signing
attestations, replaying Solana events to EVM) lives in `ts/bridge/`.
Everything liquidation-related lives in `ts/keeper/`. The server only
holds the public-facing agent metadata + auth nonce surface.

## Run

```bash
cd ts/server
npm install
npm run dev        # tsx watch on src/server.ts → http://localhost:3000
npm run typecheck
```

## Files

- `src/server.ts` — Hono app, CORS allowlist, agent card, SIWS nonce.
- `src/auth.ts` — SIWS auth middleware utility (Sign-In With Solana,
  CAIP-122). Verifies ed25519 detached signatures via `tweetnacl`. Kept
  as a utility for v1.5 endpoints; not currently mounted on any route.
- `src/pricing.ts` — off-chain mirror of the Rust fee math in
  `programs/credmesh-escrow/src/pricing.rs`. **Stays in lockstep** with
  the on-chain program: change both in the same commit.

## Env

```
PORT=3000
CREDMESH_DOMAIN=credmesh.xyz                  # SIWS message domain
SOLANA_MAINNET_RPC_URL=https://...
SOLANA_DEVNET_RPC_URL=https://...
SOLANA_MAINNET_USDC_MINT=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
SOLANA_DEVNET_USDC_MINT=4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU
SOLANA_MAINNET_ESCROW_PROGRAM=...             # set after first mainnet deploy
SOLANA_MAINNET_ATTESTOR_REGISTRY_PROGRAM=...  # set after first mainnet deploy
```
