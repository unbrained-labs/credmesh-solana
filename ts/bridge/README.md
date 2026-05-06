# credmesh-solana-bridge

Off-chain attestation bridge between the EVM lane (where reputation +
identity + governance live) and Solana credmesh-escrow (which consumes
EVM-attested credit limits to underwrite advances).

## What it does

1. **Quote endpoint (HTTP):** when an agent on Solana wants to borrow,
   they ask the bridge `POST /quote { agent_pubkey, pool_pubkey, nonce }`.
   The bridge reads EVM `ReputationCreditOracle` for the agent's current
   credit limit and outstanding exposure, signs a 128-byte
   `ed25519_credit_message`, and returns it. The agent then submits a
   Solana tx `[ed25519_verify(signed_attestation), request_advance(...)]`.

2. **Solana event tail:** the bridge listens for Solana `AdvanceIssued` /
   `AdvanceSettled` / `AdvanceLiquidated` events and replays the deltas
   back to EVM (writes via the credit-worker's settlement endpoint, or
   directly to a `MultiChainExposureRegistry` contract — TBD per
   `../trustvault-credit/` work).

## Trust model

- The bridge holds an ed25519 signing key whitelisted on Solana via
  `credmesh-attestor-registry`'s `AllowedSigner` PDA with
  `kind = AttestorKind::CreditBridge`.
- Compromised key → fraudulent attestations possible, bounded by the
  15-minute TTL on each signed message + governance revocation via
  `remove_allowed_signer` (Squads-CPI-gated).
- Multiple bridge signers may be whitelisted concurrently for redundancy
  (any-valid-sig accepted on Solana). Quorum requirement is a v1.5
  hardening.
- The signing key SHOULD be HSM- or hardware-wallet-held in production.

## Env

| Var | Required | Notes |
|---|---|---|
| `SOLANA_RPC_URL` | yes | Solana RPC (HTTP) |
| `SOLANA_WS_URL` | yes | Solana RPC (WebSocket) for the event tail's `logsSubscribe` |
| `EVM_RPC_URL` | yes | EVM RPC for ReputationCreditOracle + TrustlessEscrow reads |
| `EVM_REPUTATION_CREDIT_ORACLE_ADDRESS` | yes | EVM contract |
| `EVM_TRUSTLESS_ESCROW_ADDRESS` | yes | EVM contract for `exposure(agent)` reads |
| `BRIDGE_SIGNING_KEY_PATH` | yes | Filesystem path to a Solana-keypair-format JSON (64-byte secret+public). HSM/KMS is the v1.5 path |
| `BRIDGE_AGENT_BINDINGS_PATH` | yes | JSON file mapping `{ "<solana_pubkey_b58>": "0x<evm_address>" }`. The bridge NEVER trusts a caller-supplied EVM address; this map is the authoritative Solana → EVM identity table |
| `SOLANA_ESCROW_PROGRAM_ID` | yes | devnet `DLy82HRr…` |
| `SOLANA_ATTESTOR_REGISTRY_PROGRAM_ID` | yes | devnet `ALVf6iyB…` |
| `SOLANA_CHAIN_ID` | yes | `1` mainnet / `2` devnet (matches `ed25519_credit_message::CHAIN_ID_*`); also written into every signed attestation and verified on-chain against `pool.chain_id` |
| `BRIDGE_PORT` | no | default `4001` |
| `BRIDGE_AUTH_TOKENS` | no | comma-separated bearer tokens (`Authorization: Bearer <token>`). When unset the bridge runs token-less (rate-limit by IP) — production deployments MUST set this |
| `BRIDGE_RATE_LIMIT_BURST` | no | default `30` (per-key burst cap on /quote) |
| `BRIDGE_RATE_LIMIT_REFILL_PER_SEC` | no | default `0.2` = 12 quotes/min steady |
| `EVM_CREDIT_WORKER_URL` | no | when set, the event tail POSTs `AdvanceIssued`/`AdvanceSettled`/`AdvanceLiquidated` envelopes to `<url>/solana-event` so EVM AgentRecord state stays in sync. When unset, the tail still runs and logs the deltas locally — useful for devnet bring-up while the EVM endpoint isn't live |

## Run

```bash
npm install
npm run dev
```

## What works today vs what's pending the EVM-side handoff

**Wired:**
- HTTP `/quote` endpoint that signs ed25519 credit attestations against
  the canonical `ed25519_credit_message` layout (matches Rust verifier
  byte-for-byte).
- EVM read path via viem against `ReputationCreditOracle.maxExposure`
  + `TrustlessEscrow.exposure`. Refuses to issue attestations if any
  required env var is missing — explicit refusal beats silent fallback.
- Bridge signing key loaded from a Solana-keypair-format JSON file (64
  bytes: secret + public). Compromise-bounded by the 15-min TTL plus
  Solana-side governance revocation via `remove_allowed_signer`.

**Wired (with safe-disabled mode):**
- Solana event tail. Subscribes to escrow program logs over
  `SOLANA_WS_URL` and decodes `AdvanceIssued`/`AdvanceSettled`/
  `AdvanceLiquidated` events via the Anchor `event:<Name>`
  discriminator. When `EVM_CREDIT_WORKER_URL` is set, each event is
  POSTed as `{ version, event, slot, signature, raw_b64 }` to
  `<url>/solana-event`. When unset, the tail logs the deltas locally —
  no silent-success no-op; the operator sees them in stdout.

**Pending finalization on the EVM side:**
- The exact `<url>/solana-event` payload schema. The Solana side ships
  a stable `{ version: 1, event, ... }` envelope; the EVM credit-worker
  versions its decode of `raw_b64` independently.
