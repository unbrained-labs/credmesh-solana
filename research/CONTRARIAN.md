# CONTRARIAN — Solana-native redesign opportunities

A hunt for places where the proposed Solana port is reflexively reproducing EVM patterns when a Solana-native primitive would materially win.

**Reviewer**: Independent agent invoked with `helius:svm` + `helius:build` + 15 Exa searches (logged at the end). Came in cold; only saw the existing research package and the EVM CLAUDE.md.

**Where this and SYNTHESIS / REVIEW conflict**, this document presents the case for redesigning rather than porting. Final calls belong to the team — see "Concrete questions" at the end.

---

## Top 3 findings (highest leverage, redesign now)

**1. Mandates are Squads v4 spending limits, not off-chain JSON validated by a worker.** Squads v4 already ships exactly the abstraction CredMesh wrote from scratch on EVM: per-member, per-token, per-period auto-resetting spending caps, audited by Neodyme/OtterSec/Trail of Bits, formally verified twice. The agent's wallet becomes a Squads vault; the "mandate" is a `SpendingLimit` PDA. The chain itself enforces "agent X may only spend $Y/day on token Z." This eliminates a non-trivial chunk of `treasury.ts:buildSpendPolicy` / `validateSpend` and converts a worker-side trust assumption into an on-chain invariant. It's the single biggest "Solana primitive ate your code" win.

**2. Mandates + spend records + per-agent/per-job state belong in PDAs, not in the worker's SQLite blob.** The single-row `kv` model exists because EVM storage writes cost gas. Solana writes are basically free, parallel-execution-friendly when keyed off non-conflicting accounts, and Light Protocol's compressed PDAs drop creation cost from ~0.0016 SOL to ~0.000015 SOL — 99% cheaper. Keeping `agents`, `jobs`, `advances`, `spendRecords` in worker JSON on Solana is porting an EVM gas-optimization to a chain that doesn't need it, and you lose composability (no other program can read CredMesh state) plus you keep the single-process bottleneck. Move structured state on-chain (Anchor PDAs for `Agent`, `Advance`, `Job`; compressed PDAs or events for `timeline`).

**3. Replay protection and the "operator vs trustless" dichotomy both collapse on Solana.** The dichotomy exists because EIP-2771 relayers are awkward; Solana's fee-payer-separate-from-signer is native. Kora gives you one model: agent always signs the intent, protocol always pays SOL fees, capability scoping happens via Squads spending limits or Token-2022 delegate authorities. Replay state should be an on-chain `ConsumedPayment` PDA closed on settle (rent reclaimed) — not a worker hashmap that is also the protocol's single point of compromise. Both changes simplify the surface area and reduce the number of trust assumptions the lender makes.

## Findings table

| # | Topic | EVM-style port | Solana-native alternative | Recommendation | Confidence |
|---|---|---|---|---|---|
| 1 | In-memory worker state | Single `kv` JSON blob in SQLite | Anchor PDAs per entity + ZK-compressed timeline events | **Hybrid** (structured state on-chain, derived views off) | High |
| 2 | Operator vs trustless dichotomy | Two code paths | Single path: agent signs intent, Kora pays fee | **Redesign** | High |
| 3 | Mandates as off-chain JSON | Worker validates spend policy | Squads v4 SpendingLimit PDAs | **Redesign** | High |
| 4 | Marketplace orderbook off-chain | Bids in worker state | Manifest/Phoenix CLOB OR keep off-chain (DLOB pattern) | **Hybrid** (Drift's DLOB shape) | Medium |
| 5 | Pricing recomputed per advance off-chain | 4-component math in JS | Read curves from on-chain Pool params; keep math in program | **Port** with clamp on-chain | High |
| 6 | Reputation as worker-scored 0–100 | Single integer the worker decides | Multi-issuer SAS attestations + multiple competing ReputationScore providers (SATI pattern) | **Redesign** | High |
| 7 | Credit oracle as separate program with CPI | CPI to oracle, oracle returns max-credit | Read PDA directly via `remaining_accounts`; hardcoded curve in escrow | **Redesign** (drop the oracle program) | Medium-High |
| 8 | Receivable oracle as worker-written PDA | Worker key = single point of compromise | Payer-signed receivable verified by ed25519_program precompile | **Redesign** for x402 path; **Hybrid** elsewhere | High |
| 9 | Replay protection in worker hashmap | `consumedPayments[txHash]` | Per-receivable PDA, close on settle, rent reclaimed; reinforced by instruction-introspection of memo-nonce | **Redesign** | High |
| 10 | Repayment as worker-triggered settle | Worker sends settle tx | Single permissionless `claim_and_settle` ix; agent or anyone can crank | **Redesign** (already in synthesis) | High |
| 11 | SQLite single-process worker | Worker is source of truth | Stateless edge worker over Helius webhooks + indexer; on-chain is source of truth | **Hybrid** | Medium-High |

## Detailed treatments

### #1 In-memory worker state → on-chain PDAs (hybrid)

The synthesis preserves "single `kv` row, don't add per-record SQL." That made sense on EVM where every state mutation cost gas. On Solana the writes are free, the parallel-execution model rewards account-keyed state (each agent's writes don't contend with another's), and Light Protocol's compressed PDAs make even high-cardinality state cheap (100-byte PDA: ~$0.00135 vs $0.143 with classic accounts). **Redesign**: move `Agent`, `Advance`, `Job`, `Mandate`, `SpendRecord` to Anchor PDAs (one program owns them), and emit `timeline` entries as program events indexed by Helius webhooks → derived view. Worker becomes stateless: it composes views from chain reads + indexed events. Side benefits: no more "single-process assumption" pressure (open question #7 in SYNTHESIS), atomic dashboards, composable by other Solana protocols. Cost: more on-chain code to audit, harder to hot-patch business logic.

### #2 Operator vs trustless → one mode: signed intent + sponsored fee

EVM forced this split because gas sponsorship is awkward. Solana has Kora (audited, production, supports allowlists/blocklists/per-wallet rate limits/SPL-payment-for-fees). The agent always signs the actual instruction (a `request_advance` or `claim_and_settle`), the worker (or a self-hosted Kora signer) is fee payer. Capability scoping is done via Squads spending limits (#3) or Token-2022 PermanentDelegate on a CredMesh-issued credit token (only if you control the mint — never if it's USDC). **Redesign**: collapse `escrowIssueAdvance` and `buildAdvanceCalldata` into one server endpoint that returns a partially-signed `VersionedTransaction` with Kora as fee payer. The agent's signing UX is identical regardless of whether they're a Phantom user or a headless agent; the trust delta is now zero from the protocol's view because the agent's signature is always present.

### #3 Mandates → Squads v4 SpendingLimit (redesign)

This is where CredMesh re-implements something Solana already shipped, audited and formally verified. A Squads spending limit PDA encodes: (member, token mint, period: day/week/month/none, max_amount, allowed destinations). The chain enforces it; CredMesh's `validateSpend` becomes redundant for any agent whose wallet is a Squads vault. **Redesign**: agent registration creates (or links) a Squads vault for the agent; CredMesh writes spending limits via Squads `config_transaction` rather than storing JSON. For agents that won't migrate to a Squads wallet, keep the JSON path as a legacy fallback. Risk: forces a wallet-shape on agents (vault, not EOA), and Squads vaults cost ~0.01 SOL to set up — acceptable for a credit protocol that needs structured spending control.

### #4 Marketplace orderbook (hybrid — copy Drift's DLOB)

Manifest gets to ~0.004 SOL per market and 45% less CU than Phoenix; per-order tx cost on Phoenix is sub-cent. Putting bids fully on-chain is now affordable for a job marketplace. **But** Drift consciously moved its order book *off* chain (DLOB run by keepers, on-chain only for matching settlement) because tx packing + write-lock contention on a single market account hurt throughput. CredMesh's volume is way lower than Drift's, so on-chain is fine — except the ergonomics aren't great: every bid/cancel is a tx, agents need SOL or Kora. **Recommendation**: hybrid. Job listings + awards on-chain (anyone can audit); bids stay off-chain in worker state (cheap to express, low stakes), with the *winning* bid committed on-chain at award time. This matches DLOB shape and is what the marketplace actually wants — the bid book is ephemeral, the award is the durable artifact.

### #6 Reputation: composition of attestations, not a single integer (redesign)

EVM's "worker scores 0–100" model assumes one trusted scorer. Solana has SAS attestations (stable since 2025-05) and the SATI/8004 pattern of multiple competing `ReputationScoreV3` providers per agent — applications choose which providers they trust. **Redesign**: replace `writeReputation` with three primitives: (a) post-job `FeedbackPublicV1` attestations (anyone, weighted by client signature when the client is the actual job authority), (b) a CredMesh-published `ReputationScore` provider that aggregates feedback into the integer the credit oracle reads (same shape, but composable), (c) optional KYC/sanctions attestations from Range/RNS via SAS as a gating tier. Net effect: CredMesh stops being the *only* reputation oracle for its agents — other lenders, payment apps, and explorers can read the same shape. The `ReputationScore` PDA the credit oracle dereferences is still owned by CredMesh's provider; only the *substrate* changes.

### #7 Credit oracle: drop the separate program, read PDA directly (redesign)

The synthesis devotes a section to CPI + `set_return_data` for credit decisions, citing Drift. Drift uses CPI for *price oracle source dispatch* (Pyth vs Switchboard vs internal AMM), not for computation — it's a switch, not a function call. CredMesh's "compute_credit_from_reputation" is deterministic math over a `ReputationScore` integer plus a curve. The escrow program can read the reputation PDA via `remaining_accounts` (no CPI tax, no 4-deep CPI ceiling consumed) and apply the curve in-program. The `reputation_credit` oracle program adds an instruction call with no security benefit since the reputation PDA is the source of truth and the curve is governance-tunable as a Pool param. **Redesign**: delete `reputation_credit` as a separate program. The receivable oracle (where the data origination *is* the trust boundary) stays as a separate program because the writer authority matters; the credit oracle does not.

### #8 Receivable oracle: payer-signed receivable, ed25519_program-verified (redesign for x402)

This is the weakest single point of trust in CredMesh per the original prompt. On EVM, fixing it required L2 attestations or zk proofs. On Solana you can hand the agent a *payer-signed* attestation ("Hyperliquid signed: address X has $Y in receivable Z") and verify it on-chain via the ed25519 native program (precompile, top-level instruction, the escrow uses instruction-introspection at index `current-1` to confirm the verification ran with the expected pubkey/payload). This is exactly the x402 facilitator pattern in inverse: instead of the merchant verifying that Circle's USDC transfer landed, the lender verifies that Hyperliquid's signing key attested to the receivable. **Redesign**: where a payer publishes a stable signing key (any major exchange, Pyth Lazer-published HL feed if it lands, x402 servers themselves), the receivable oracle is *not* worker-written; it's payer-signed and verified in one instruction. The worker key only writes for sources without public keys (legacy webhooks). This shrinks the "worker key compromise = inflated receivables" attack surface to the legacy long-tail.

### #9 Replay protection on-chain (redesign)

The synthesis correctly notes `consumedSignatures[sig]` is insufficient (an attacker can re-wrap a `TransferChecked` ix in a different outer tx) and proposes `keccak(sig||receivable_id)` — REVIEW.md correctly criticizes this since both inputs are attacker-influenced. **Real Solana-native solution**: per-receivable `Consumed` PDA seeded by `["consumed", receivable_id]`. `init` semantics — second attempt fails atomically because the account already exists. On settle, the PDA closes and rent goes back to the agent (~0.002 SOL recovered per receivable). Plus a cheaper layer: at advance issuance, the worker hands the agent a server-issued nonce; the payment tx must include a memo with that nonce; the escrow uses instruction-introspection to verify the memo is in the same tx. Combined: on-chain idempotence + nonce binding eliminates the "same TransferChecked re-wrapped" attack. Move the entire `consumedPayments` map out of worker state.

### #10 Repayment: permissionless `claim_and_settle` (redesign — synthesis already aligned)

The SYNTHESIS proposes this. I confirm: don't reproduce the worker-triggered EVM model. Make `claim_and_settle` permissionless after a short post-payment time-lock (any keeper, agent, or LP can crank), with the waterfall (15% protocol → 85% LP-vault including principal repay → agent net) as a single instruction. Important caveat: the elegant "transfer hook = waterfall" idea is **off the table** — Token-2022 transfer hooks pass source/destination as read-only accounts and cannot mutate amounts or split transfers. Hooks can gate, not divide. So a single Anchor instruction is the right shape; CascadePay is the closest reference (read but don't fork — their 1% fee is hardcoded).

### #11 SQLite worker: stateless edge over webhooks + indexer (hybrid)

Once #1 lands and structured state is on-chain, the worker becomes mostly a Hono router that: (a) builds + partial-signs transactions, (b) ingests Helius webhooks for state derivation, (c) exposes views computed on demand from chain reads + an indexer. The single-process SQLite remains as a *cache* for derived views (timeline, search), not as the source of truth. Eliza, ATOM, and 8004scan all use this shape: on-chain programs are canonical, off-chain is enrichment. This dissolves the "single-process bottleneck" worry (REVIEW.md soft-spot #4) without forcing a rewrite to multi-process.

### Items recommended to PORT (no redesign)

- **#5 Pricing**: 4-component math is fine off-chain at advance time. Pyth Lazer is for market prices, not protocol fee curves. Keep `pricing.ts` logic; just store curve parameters as Pool PDA fields so governance can tune them via timelock. The 400ms slot vs 2s block doesn't materially change the model — duration is wall-clock seconds in both (REVIEW.md confirms).
- **#4 Marketplace** (hybrid as above): partial port.

## Risks introduced by going Solana-native

- **Squads dependency** (#3): you tie spend-limit semantics to a specific protocol's account layout. Squads v4 is well-audited, but a future v5 migration is on you. Squads also has its own time-locks/governance you'd need to learn.
- **Compressed PDAs / ZK-compression** (#1): Light Protocol is open-source but the indexer (Photon) is the read path. If Helius's Photon endpoint goes down, your dashboard goes blind even though state exists on-chain. Dual-index (self-host Photon for read-side resilience).
- **SAS / multi-issuer reputation** (#6): more flexible but more attack surface — Sybil-issuer attestations are cheap to mint. The credit oracle's *filter* (which providers it trusts) becomes a governance-managed allowlist, which adds operational burden CredMesh didn't have before.
- **ed25519 precompile receivables** (#8): instruction introspection is finicky (use relative index, not absolute, per RareSkills warning). Audit risk concentrates here. Also: one ed25519 verification is 5000 lamports + the verification instruction itself, so it's not free per advance.
- **On-chain mandates** (#3): forces agents into Squads vaults — adds onboarding friction. Some agents may refuse; need a fallback.
- **Permissionless `claim_and_settle`** (#10): MEV bots may crank settles to capture rent on the closing PDA. Set the rent recipient = agent, not crank-caller, to neutralize.
- **Stateless worker** (#11): debugging is harder. The "look at the worker's state" diagnostic flow becomes "look at the chain + indexer + worker view-cache."

## Concrete questions for the user before picking redesign vs port

1. **Are your agents OK being Squads vault holders?** If yes, #3 is a no-brainer. If many will be plain EOA agents, mandates need to stay JSON for that path.
2. **Will receivables ever come from sources without a public signing key?** If yes, #8 needs the worker-written fallback path; if all sources are webhooked exchanges or x402 servers, you can purify.
3. **How important is third-party readability of CredMesh state?** If other Solana protocols should be able to read advances/jobs/reputation directly, #1 and #6 are wins. If CredMesh is closed-loop, the off-chain blob is fine.
4. **What's the audit budget?** #1 + #3 + #6 + #8 each materially expand the on-chain surface. Cheaper to port to Solana with minimal redesign and audit a thin program; more capable to redesign per above and audit more.
5. **Do you want to keep the dashboard's "single-process timeline" semantics?** If yes, the SQLite blob can stay as a derived-view cache (#11 hybrid). If not, you can go full edge-stateless.
6. **Is the credit oracle's curve ever expected to be non-trivially complex (ML-derived, multi-signal)?** If yes, keep it as a separate program (#7 stays). If it's "tier-curve over a single integer," delete it.
7. **Solana Mobile / mobile-wallet-adapter agents in the roadmap?** Affects #2 because Mobile Wallet Adapter has different signing constraints than browser wallets.

## Exa search log

1. `Squads v4 spending limits per-member periodic reset Solana on-chain capability mandates 2025`
2. `Token-2022 transfer hook fee splitting waterfall payment routing Solana 2025`
3. `Solana account compression Bubblegum Light Protocol ZK compression high-volume timeline events 2025`
4. `Phoenix OpenBook Solana on-chain CLOB order book costs throughput credit lending order matching`
5. `Solana Attestation Service SAS attestations agent reputation composable signed claims 2025`
6. `Solana Token-2022 PermanentDelegate session keys agent autonomous spending 2025`
7. `Streamflow Drift Squads on-chain state vs off-chain backend Solana protocol architecture topology`
8. `Solana ed25519 signature verification on-chain ed25519_program syscall payer attestation receivable proof`
9. `Solana fee payer separate from signer gasless relayer Kora EIP-2771 equivalent agent`
10. `Pyth Lazer 1ms price feed pull oracle Solana on-chain dynamic interest rate fee curve 2025`
11. `Sealevel parallel execution writeable account contention DeFi protocol design lessons 2025`
12. `Sendai Eliza Olas Solana AI agent backend architecture stateless event-driven`
13. `Solana program memo nonce instruction introspection replay protection sysvar instructions`
14. `Loopscale Solana credit lending order book per-loan PDA repayment ledger architecture 2025`
15. `Solana on-chain reputation aggregator multi-issuer attestation composable 8004 cred protocol agents`
