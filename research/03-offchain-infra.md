# 03 — Off-chain Backend & Frontend

**Scope**: Auth, tx submission, multi-cluster config, real-time eventing, USDC handling, dashboard wallet adapter. Solana equivalents of `chain.ts`, `chains.ts`, EIP-191 auth middleware, and the wagmi/viem dashboard.

**Method**: Research agent invoked Helius skills (`build`, `phantom`, `svm`) and Exa for SIWS, Sender, web3.js v2 / Kit, ConnectorKit.

---

## Auth model

Replace EIP-191 `personal_sign` with **Sign-In With Solana (SIWS)**, the Solana-native equivalent of EIP-4361. It standardizes a human-readable message format (domain, address, statement, URI, version, nonce, issuedAt, expirationTime, chainId, requestId, resources). Phantom and Solflare render SIWS messages with anti-phishing UI when they detect the format; older wallets fall through to plain `signMessage`.

For CredMesh, keep parity with the current shape but make the body SIWS-compatible:

- Header `X-Agent-Address`: base58 ed25519 pubkey (32 bytes).
- Header `X-Agent-Signature`: base58 detached ed25519 signature (64 bytes).
- Header `X-Agent-Timestamp`: ISO 8601 (matches `issuedAt` in the SIWS message, easier to enforce a +/- 60s skew).
- Header `X-Agent-Cluster`: `mainnet-beta` | `devnet` (analog of `chainId`, prevents cross-cluster replay).
- Optional `X-Agent-Nonce`: 16 alphanumeric chars when you want server-issued nonces (recommended for write endpoints; the worker hands out a nonce, client signs SIWS payload that embeds it, server matches and burns).

The signed payload should be the canonical SIWS string, not just `credmesh:{address}:{ts}` — wallets warn users about non-SIWS messages, and cross-cluster/domain replay is a real concern. Include `domain: credmesh.xyz`, `statement: "Authenticate to CredMesh"`, `cluster`, `nonce`, `issuedAt`, `expirationTime` (issuedAt + 5 min).

**Verification** uses `tweetnacl`:
- decode signature (base58 -> Uint8Array(64))
- decode pubkey (base58 -> Uint8Array(32))
- `nacl.sign.detached.verify(messageBytes, sigBytes, pubkeyBytes)`

`assertAuthorized(verifiedAddress, target)` stays unchanged — it's just a string compare on base58 pubkeys. Use `@solana/kit`'s `address()` for type safety.

Phantom embedded wallets (Google/Apple OAuth) only support `signAndSendTransaction`, not detached `signMessage` for arbitrary bytes — fall back to a server-signed challenge or use SIWS via the supported path. The Phantom Connect SDK exposes `signMessage` for injected (extension) wallets which covers most CredMesh agents.

## Tx submission stack

**Use Helius Sender for everything.** It dual-routes to validators + Jito and is free (no credits). Plain RPC `sendTransaction` is the wrong default; Jito-only also misses the validator path. Operator-mode worker should POST to the HTTPS endpoint:

- Endpoint: `https://sender.helius-rpc.com/fast` (browser-safe, no API key).
- Required: `skipPreflight: true`, `maxRetries: 0`, app-side retry loop.
- Required: minimum **0.0002 SOL Jito tip** to one of the 10 published tip accounts (or 0.000005 SOL on the SWQOS-only path via `?swqos_only=true` if the worker doesn't need MEV protection).
- Required: `ComputeBudgetProgram.setComputeUnitPrice(microLamports)` + `setComputeUnitLimit(units)`.
- Instruction order: CU limit, CU price, business instructions, tip transfer last.

**Priority fees** via Helius `getPriorityFeeEstimate` with `accountKeys` passed in (so the estimate reflects the writable accounts in your tx). Recommend per call type:

| Call class | Priority level | Rationale |
|---|---|---|
| advance issuance, settlement waterfall | `High` | money path, must land first attempt |
| reputation writes, identity registry | `Medium` | non-time-critical |
| faucet / test mints | `Low` | devnet, cost-sensitive |

Fetch fees right before signing (they go stale fast). Cache for ~2s if you're sending in bursts. Hardcode nothing.

For higher TPS (the "hundreds/sec at peak" target), request custom TPS limits from Helius (default 50 TPS per region) and use the per-region HTTP Sender endpoints from the worker (not browser): lowest latency, no CORS concerns server-side. Keep an idle GET to `/ping` to keep connections warm.

## Trustless calldata builder

Two approaches; **return a base64-encoded unsigned `VersionedTransaction`** with the agent listed as fee payer and the worker's signature pre-applied if a multi-sig pattern is needed. Reasons:

- A Solana tx is structurally `(message, [signatures])`. Unlike EVM calldata, the client can't construct it from "instruction bytes + accounts" without knowing the recent blockhash, fee payer, ALTs, and CU budget — all of which the worker is better positioned to compute.
- The worker already has Helius RPC for blockhash and `getPriorityFeeEstimate`; it's where the policy lives.
- Wallet UX is better with a fully prepared tx (Phantom shows a clean simulation).

Recommended endpoint shape: worker returns `{ tx: base64, lastValidBlockHeight, expectedSignerPubkey }`. The agent decodes, calls `signTransaction`, and submits via Sender. If there are co-signers (e.g., the protocol must also sign for an oracle update), the worker partial-signs first, sets `feePayer = agent`, serializes with `requireAllSignatures: false`, and the client adds its signature without modifying the message bytes (any modification invalidates prior signatures).

**Anchor vs raw web3.js v2 / `@solana/kit`**: For new code in 2026, use `@solana/kit` + `@solana-program/*` clients + Codama-generated clients for any custom programs. Anchor's TS client is still pinned to web3.js v1; if the on-chain side is Anchor, generate a Codama client off the IDL rather than using `@anchor-lang/core`. The off-chain backend has zero reason to take the Anchor TS dep — pick `@solana/kit` and import program clients from `@solana-program/system`, `@solana-program/token`, `@solana-program/compute-budget`, etc. The Helius SDK's `helius.raw` is a Kit-compatible `Rpc` client.

## Multi-cluster config

Solana has fewer dimensions than EVM (no chain ID per se), but more provider-specific endpoints. CredMesh's `{PREFIX}_*` env-var pattern translates cleanly:

```
{PREFIX}_CLUSTER          # "mainnet-beta" | "devnet" | "testnet"
{PREFIX}_RPC_URL          # Helius RPC (api-key in URL, kept server-side)
{PREFIX}_SENDER_URL       # https://sender.helius-rpc.com/fast or regional HTTP
{PREFIX}_WS_URL           # Enhanced WS (server-relayed; never browser-direct)
{PREFIX}_OPERATOR_KEYPAIR # base58 secret key for protocol wallet
{PREFIX}_USDC_MINT        # mainnet: EPjFW...wyTDt1v / devnet: 4zMMC...DncDU
{PREFIX}_ESCROW_PROGRAM   # program ID (will differ per cluster)
{PREFIX}_VAULT_ADDRESS    # PDA or account address
{PREFIX}_REPUTATION_PROG
{PREFIX}_IDENTITY_PROG
{PREFIX}_TIP_ACCOUNTS     # CSV of Jito tip accounts (optional override)
```

Suggested prefixes: `SOLANA_MAINNET_*`, `SOLANA_DEVNET_*`. The `getClients()` priority pattern still applies (devnet first when both are set in dev environments). Keep the legacy EVM `{PREFIX}_*` registry alive in parallel during the transition — the chain-aware abstraction in `chains.ts` is conceptually identical, just gains a `kind: "evm" | "svm"` discriminant.

Important: program IDs **will differ across clusters** because each redeploy generates a new program ID unless you use a fixed buffer + program upgrade authority. Treat program ID as required env per cluster, not derivable.

## Real-time eventing

Three options ranked for CredMesh's needs (vault deposits, advance issuance, repayment, default):

1. **Helius webhooks (primary).** Push HTTP POSTs to the worker filtered by `accountAddresses` (vault PDA, escrow program, agent token accounts). Best fit for a durable backend pipeline:
   - 24h retry with exponential backoff
   - Auto-disable at >=95% delivery failure (with email alerts on Dev+)
   - Free / low credit cost
   - Deduplicate by tx signature in `state.consumedSignatures`
   Use `webhookType: "enhanced"` to get parsed transaction types (TRANSFER, SWAP, etc.) without re-parsing.

2. **Enhanced WebSockets (secondary).** Sub-second `accountSubscribe` / `transactionSubscribe` for the dashboard's live UI. Server-side connection, relay to browser via SSE — never open the WS from the browser (API key sits in the URL). 1.5–2x faster than standard WS.

3. **Laserstream gRPC (only if warranted).** Lowest-latency replay-capable stream, but Business+ plan and operationally heavier. Overkill until volumes justify it; revisit if the worker needs to react to mempool-level events for liquidations.

**Recommended setup:** webhooks for the engine's source-of-truth state transitions; WS-relay-via-SSE for the dashboard timeline; skip Laserstream initially. Webhooks are at-least-once — keep the existing `consumedPayments` dedupe logic, just rename to `consumedSignatures`.

## Replay protection

Solana tx signatures are unique by construction (Ed25519 over a message that includes the recent blockhash + nonce-style account state). The existing `consumedPayments[txHash] = true` map ports verbatim to `consumedSignatures[signature]`. One subtlety: `recentBlockhash` is valid for **150 slots (~80–90s)**. After that window, an unconfirmed tx is dropped by the cluster — but a *confirmed* tx is permanently in the ledger and the signature stays unique forever, so verification windows don't change. Just don't try to verify a tx the agent claims to have submitted if you can't fetch it via `getTransaction` after ~2 minutes — it likely never landed.

For long-deferred payment proofs (the worker checks a payment hours later), use **durable nonces** server-side instead of recent blockhash. The agent's tx then has no expiry; the worker can verify the signature whenever convenient. Keep this in your back pocket; not needed for V1.

## USDC handling

- Mainnet USDC mint: `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` (confirmed via Circle docs). 6 decimals — same as EVM USDC, so the existing `parseUnits(value.toFixed(2), 6)` rounding via `rc()` carries over.
- Devnet USDC mint: `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU` (Circle's official testnet). 6 decimals.
- Both are **classic SPL Token** (program `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA`), not Token-2022. Don't reach for the `@solana-program/token-2022` client unless CredMesh moves to a custom mint.
- USDC requires an **Associated Token Account (ATA)** per holder per mint. The worker must derive ATAs (`getAssociatedTokenAddress`) and lazily create them via `createAssociatedTokenAccountIdempotent` — costs ~0.002 SOL rent each, payable by the worker in operator mode.
- Faucet analog for `mintTestTokens`: Circle's public testnet faucet at `faucet.circle.com` (Solana Devnet, 20 USDC per request, 2hr cooldown). For programmatic dev test fixtures, deploy your own test mint on devnet (you control the mint authority, no rate limit) — same pattern as `TestUSDC.sol`. Keep both: real Circle USDC for end-to-end realism, self-mint for CI.

## Frontend wallet integration

**Recommendation: Phantom Connect SDK + Solana ConnectorKit fallback.** Specifics:

- Use **`@phantom/react-sdk`** as the primary entry. It supports the extension *and* embedded wallets (Google/Apple OAuth via Phantom Portal), plus `@solana/kit` types natively. This is the closest thing to wagmi+RainbowKit on Solana.
- For multi-wallet (Solflare, Backpack, Glow, WalletConnect, Mobile Wallet Adapter), use Solana Foundation's **ConnectorKit** (`@solana/connector`, released 2025-09). It's Wallet Standard-first, has React hooks, framework-agnostic core, mobile support, and supports both `@solana/kit` and legacy web3.js. This is the modern replacement for `@solana/wallet-adapter-react` (which is still v1-only).
- Skip raw `window.phantom.solana` — it's the legacy provider, requires web3.js v1 types, and doesn't work with `@solana/kit`.

**Migration deltas from the existing wagmi/viem dashboard:**

| EVM hook | Solana equivalent |
|---|---|
| `useAccount()` | `useAccounts()` (Phantom) or `useConnector()` (ConnectorKit) |
| `useSignMessage()` | `useSolana().signMessage()` |
| `useSendTransaction()` | `useSolana().signTransaction()` + POST to Helius Sender |
| `usePublicClient()` | Helius Kit `Rpc` from `helius.raw` (server-proxied) |
| `useWalletClient()` | not needed — Phantom holds the keys, sign-only flow |
| `useReadContract()` | `getAccountInfo` / Codama-generated account fetchers |
| `useWatchContractEvent()` | SSE relay from server-side Helius `accountSubscribe` |

Critical security shift: the worker proxies *all* Helius API calls except Sender. `NEXT_PUBLIC_HELIUS_API_KEY` is forbidden — the API key must stay server-only. Sender is the one endpoint safe to call from the browser because it doesn't take an API key.

## MCP server

Confirmed: nothing in `packages/mcp-server` is chain-specific. It already calls the public CredMesh HTTP API and stays HTTP-API-shaped post-port. The only change needed is its README/agent-card descriptions of which chains/clusters CredMesh supports — text edits, no logic changes.

## Open questions

1. **Compressed accounts / state compression**: worth it for the per-agent state (mandates, timeline)? Cuts rent costs at scale but adds Merkle proof complexity. Probably not for V1.
2. **Pyth vs Switchboard for oracle reads** (parallel to your ERC-8004 reputation oracles) — not in scope for the off-chain port, but the worker may need to read price feeds.
3. **Mobile Wallet Adapter (Solana Mobile Stack)** — does CredMesh need first-class Saga/Seed Vault support, or is web wallet sufficient for agents?
4. **Phantom Portal App ID acquisition**: who in your org owns this account? Required for Google/Apple OAuth login on the dashboard.
5. **Per-cluster program upgrade authority**: are you using a single program keypair across mainnet/devnet (same program ID) or separate deploys? Affects env-var parity with EVM.
6. **Webhook authentication**: Helius supports an auth header secret per webhook; ensure CredMesh's `authMiddleware` whitelists the webhook ingress route or uses a separate `X-Helius-Auth` check.
7. **Single-process state assumption**: the SQLite single-row blob model assumes one worker. Solana's higher event volume (subsecond webhooks, possibly 10s/sec at peak) may pressure that — measure before scaling out.

## Key references

1. https://siws.web3auth.io/spec — SIWS message specification
2. https://docs.phantom.app/solana/signing-a-message — Phantom signMessage + tweetnacl verify
3. https://www.helius.dev/docs/sending-transactions/sender — Sender requirements (tip, priority fee, skipPreflight)
4. https://www.anza.xyz/blog/solana-web3-js-2-release — web3.js v2 / Kit migration story
5. https://solana.stackexchange.com/questions/16703/can-anchor-client-be-used-with-solana-kit — Anchor + Kit via Codama
6. https://developers.circle.com/stablecoins/usdc-contract-addresses — USDC mainnet/devnet mints
7. https://github.com/solana-foundation/connectorkit — multi-wallet Solana adapter (Wallet Standard)
8. https://solana.com/developers/guides/advanced/introduction-to-durable-nonces — 150-slot expiry + durable nonce escape hatch
