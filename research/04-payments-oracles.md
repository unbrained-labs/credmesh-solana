# 04 — Payment Rails, Oracles & Intelligence

**Scope**: Solana equivalents of x402 / EIP-3009, fiat ramps, receivable oracles (Registry, Hyperliquid, DEX-derived), atomic repayment splitting, cross-chain payment, MEV.

**Method**: Research agent invoked Helius skills (`okx`, `dflow`, `jupiter`, `build`) and Exa search.

---

## Gasless / x402-equivalent on Solana

**Recommendation: Use x402 natively — it already supports Solana.** As of late 2025, x402 is no longer EVM-only. Coinbase + community shipped Solana SVM support with the same `X-PAYMENT` / `402 Payment Required` semantics; the protocol processes 75M+ tx/month including Solana flows. The mechanics differ mechanically from EIP-3009 because SPL has no `transferWithAuthorization`, but the developer-facing flow is identical.

How Solana x402 actually works under the hood:
- Server returns `402` with payment requirements (asset = USDC mint `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v`, network = `solana`).
- Client builds a Solana transaction containing a USDC SPL `TransferChecked` to the merchant ATA, and **partially signs** it (user signs as token authority, fee payer slot left for facilitator).
- Client sends serialized partial-tx in `X-PAYMENT`. Facilitator (Kora signer node, AnySpend, PayAI, Corbits, or self-hosted) co-signs as fee payer, submits via Helius Sender, returns settlement proof.
- Server's `verifyPayment()` decodes the signature, confirms the USDC transfer event landed, dedupes by signature.

This is not EIP-3009 in spirit — it's a **partial-sign + relayer** pattern, semantically equivalent. Token-2022 transfer hooks and `PermanentDelegate` are the wrong tool: hooks are read-only at the signing layer (PrivilegeEscalation errors when you try to mutate), and `PermanentDelegate` requires the mint itself to enable it (USDC will not). Use them only if CredMesh issues its own credit-token. Solana Pay URI is the wrong abstraction here — it's user-facing QR-code intent, not machine-to-machine HTTP.

**Concrete recommended stack:** Kora (gasless facilitator) + Helius Sender for landing + Helius `parseTransactions` for `verifyPayment()` semantics (replay dedup via signature).

Alternatives if Kora/Coinbase facilitators don't fit:
- **AnySpend (B3)** — production Solana x402 facilitator, mainnet endpoint live.
- **PayAI / Corbits / MCPay.tech** — similar facilitators with Solana support.
- **Self-hosted Kora** — full control; CredMesh's protocol wallet becomes the fee payer.

## Fiat on-ramp/off-ramp options

| Provider | Direction | KYC | Fees | Solana USDC support | Notes |
|---|---|---|---|---|---|
| **Coinbase Onramp** | Buy → Solana | Coinbase handles, optional guest checkout in US | **0% on USDC** (apply for no-fee) | Yes | Best for buy-side; offramp requires linked Coinbase account, no guest |
| **Stripe Crypto Onramp** | Buy → Solana | Stripe is MoR, full KYC | ~1.5% + payment-rail fee | USDC (Solana) yes (US only — not EU) | Easy embed; no offramp on Solana |
| **MoonPay** | Buy + Sell ↔ Solana | KYC every user | 1–4.5% | Yes both directions | Higher fees, broadest geo |
| **Coinbase Offramp** | Solana → fiat | Coinbase account required | Free for USDC | Yes | Only path with cheap offramp to Solana |
| **Bridge / Stripe Issuing** | USDC payouts | Business KYB | ~1% | Yes via partner stack | For protocol-side payouts to LPs, not agents |
| **DFlow Proof KYC** | N/A — identity layer | Docs + selfie | N/A | N/A | Not a ramp; binds wallet to verified identity for Kalshi-style compliance |

DFlow's Proof KYC is **not** a ramp — it's a wallet-to-identity graph. Use it as the agent-tier gate (e.g., higher credit limits for KYC'd agents) rather than as a fiat rail. There is no equivalent of "Tempo MPP" on Solana today; the closest substitute is **Coinbase Onramp + CredMesh-controlled Solana wallet** that auto-credits agent accounts on receipt.

## Receivable oracle architecture on Solana

Three oracle classes from the EVM design map cleanly:

1. **RegistryReceivableOracle (canonical, ship this first).** Worker writes a per-agent PDA: `seeds = [b"receivable", agent_pubkey]`. Account holds `(amount_usd_micros, expiry_slot, last_updated_slot, source_id)`. The on-chain advance program reads it directly via `account_info`. No innovation needed — this is the same trust model as the EVM `RegistryReceivableOracle`. The worker is the writer; oracle attestation is by design.

2. **DEX-derived receivables — does not exist on Solana, by design.** Jupiter swaps, OKX swaps, and DFlow spot trades are **atomic** — settlement is in the same transaction. There is no "pending receivable" state to read. Don't try to port this; it's an EVM artifact of async order books. The right substitute: **Jupiter Trigger limit orders** and **Recurring DCA** programs do hold pending state (escrowed input token + price target), readable via `getProgramAccounts` on the Trigger program. If an agent's "income" is a Trigger-based limit sell, the unfilled-order amount × Pyth price is a legitimate receivable.

3. **Hyperliquid receivable on Solana — read cross-chain via Pyth Lazer / CCTP attestation.** Hyperliquid HL1 has no native Solana view. Two paths:
   - **Worker-attested:** worker reads HL via Hyperliquid API, signs, writes to Solana PDA. Same trust as `RegistryReceivableOracle`. Ship this.
   - **Pyth Lazer cross-chain:** Lazer can carry custom signed payloads at 1ms-200ms cadence, verified on-chain via the Solana ed25519 program. If Hyperliquid becomes a Lazer publisher (or you publish via your own Lazer feed), Solana programs can verify HL state with cryptographic attestation. Lower trust than worker-attested, much higher op cost.

Pyth Pro/Lazer itself is **strictly market data** (prices, bids, asks, funding rates). It does **not** serve credit, volume, or reputation feeds. Don't expect Pyth to replace the reputation oracle; it complements price-marked receivables only.

## Credit-scoring data pipeline

Solana wallet intelligence is **substantively richer** than EVM equivalents and should reshape (not just port) the reputation oracle. Pipeline tiers:

**Tier 1 — Identity & provenance** (gating signal, not score):
- `getWalletIdentity` (Helius) — labels exchanges, protocols, KOLs from a 12,500-label DB. Gate: known mixer / sanctioned cluster → reject.
- `getWalletFundedBy` — first SOL transfer source. Funded by Binance/Coinbase = real-money provenance. Funded by an unknown wallet = inherit that wallet's risk recursively (cap at depth 3).
- DFlow Proof KYC `/verify/{address}` — boolean verified identity. Tiered cap: unverified $25, verified $250.

**Tier 2 — Cashflow signals** (positive credit input):
- `getWalletHistory` (Helius, 90-day window) — count and median size of stable inflows (USDC/USDT/PYUSD). Use sustained inflow ≥ $X for ≥ N days as the credit ceiling proxy. EVM had no equivalent of this granularity.
- `getWalletTransfers` filtered to USDC mint — separates "earned" inflows from token-swap noise.
- `parseTransactions` over Jupiter/DFlow program IDs — counts successful trade fills as productive activity.

**Tier 3 — Smart-money signals** (bonus tier, cap-raising):
- OKX smart-money endpoints — flag wallets the OKX cluster identifies as profitable traders or KOLs. Treat as bonus credit, never as the only signal.
- DFlow agent CLI history — if the agent is registered, its trade history is a directly observable performance record.

**Composite score**: `base (reputation history) + min(tier2_cashflow_cap, tier3_bonus_cap) × tier1_multiplier`. Cap each tier separately so any single signal failure (e.g., funded-by depth exceeded) caps but doesn't zero the score.

Helius costs: `getWalletHistory` ≈ 100 credits, `getWalletIdentity` 1 credit, `getWalletFundedBy` 100 credits. Cache scores in worker state with 24h TTL; recompute on advance request.

## Atomic repayment splitting pattern

**Recommended pattern: single Anchor instruction `claim_and_settle` on a per-job escrow PDA.** Inputs: job PDA (authority), payer USDC ATA, agent USDC ATA, protocol-fee USDC ATA, LP-vault USDC ATA, oracle account (for fee math). The instruction:

1. Asserts caller is job authority or post-payment time-locked permissionless.
2. Reads waterfall amounts from the job PDA (principal_owed, accrued_fees, late_penalty).
3. CPIs three SPL `TransferChecked` instructions in sequence: protocol cut → LP vault → agent net.
4. Closes job PDA on full settlement; on partial, decrements amounts and re-marshals.

All three transfers must be in **the same instruction** (not just same tx) so a downstream CPI can't squeeze between them.

**Reference protocols on Solana that do this well:**
- **CascadePay** (`Bi1y2G3hteJwbeQk7QAW9Uk7Qq2h9bPbDYhPCKSuE2W2`) — production payment splitter, 2-20 recipients per config, x402-facilitator-aware (auto-detects split vault and bundles transfer + execute_split atomically). Closest direct analog to CredMesh's waterfall. **Read this contract before writing your own.**
- **Streamflow** — on-chain vesting/payouts, audited (FYEO/OPCODES), supports per-second streaming and conditional unlocks. Useful for the LP-side accrual; overkill for atomic split.
- **Squads v4 Recipients** — recurring multi-recipient batch payouts, multisig-gated. Right model for treasury operations, wrong shape for per-job claims.
- **Meteora Dynamic Vaults** — useful for the LP idle-yield layer (auto-rebalance USDC across lending venues), not for splitting.

Build the splitter; don't fork CascadePay (their 1% protocol fee is hardcoded and won't match CredMesh's 15%).

## Cross-chain payment story

Today: **Solana CredMesh advances credit, agent pays EVM x402 server, settle via CCTP v2.** This is now frictionless in 2026:
- Circle's CCTP v2 + Bridge Kit auto-forwarding launched on Solana (March 2026), covering 14+ chains including Base, Ethereum, Arbitrum.
- Burn USDC on Solana → mint on Base → call EIP-3009 `transferWithAuthorization` against the EVM x402 server. End-to-end ~30s with auto-relayer (Wormhole CCTP route or Circle's native).
- For the agent UX: bridge in advance during credit issuance ("CredMesh credit available on chain X"), or JIT-bridge per invoice using Wormhole's `cctpExecutorRoute` with native gas drop-off.

Going forward: **x402 servers are landing on Solana natively** (Coinbase's reference impl, Cloudflare partnership, AnySpend in production). Within 6-12 months expect the dominant agent-pays-tool flow to be Solana-native x402 with no bridge. Build the EVM bridge fallback; don't make it the primary path.

## Open questions

- **Receivable freshness vs cost.** Worker-written PDAs need write tx cost (~5000 lamports). At scale this is meaningful; consider batched receivable updates via Anchor account compression or a Merkle-rooted receivable tree.
- **Replay protection on Solana x402.** EVM uses `paymentTxHash` dedup. Solana tx signatures are 64 bytes — does the existing `consumedPayments` map need a different key length? Confirm during port.
- **Token-2022 USDC.** Circle has not migrated USDC to Token-2022 yet. If they do, transfer-fee extension changes the waterfall math — fees come off the top before split. Watch the Circle changelog.
- **DFlow Proof for agents.** Proof's KYC is selfie+ID — works for human-operated agents, blocks fully autonomous AI agents that have no human. Is there an "agent identity" tier in Proof's roadmap? Open question for the DFlow team.
- **MEV exposure on protocol swaps.** If the LP vault auto-rebalances or skims fees via Jupiter, default to Jupiter routes with Jito Bundle tipping (`jitodontfront` accounts) and Helius Sender's dual-route. RFQ-only is wrong here — needs price-impact-aware routing across pools, which the public router gives you.
- **Pyth Lazer Hyperliquid feed availability.** Confirmed Lazer supports custom payloads + Solana ed25519 verification, but is HL itself a Lazer publisher? If not, this path requires CredMesh to run the publisher, which is significant ops.

## Key references

- x402 Solana: https://solana.com/developers/guides/getstarted/intro-to-x402
- x402 Kora facilitator: https://solana.com/developers/guides/getstarted/build-a-x402-facilitator
- x402 networks/tokens: https://docs.x402.org/core-concepts/network-and-token-support
- AnySpend Solana x402 (production): https://docs.b3.fun/anyspend/x402-quickstart-solana
- Pyth Pro/Lazer Solana SDK: https://docs.pyth.network/price-feeds/pro/integrate-as-consumer/svm
- DFlow Proof KYC integration: https://pond.dflow.net/build/proof/partner-integration
- CascadePay payment splitter (x402-aware): https://cascadepay.io/
- CCTP v2 on Solana: https://www.circle.com/cross-chain-transfer-protocol
