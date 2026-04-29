# DESIGN — CredMesh Solana programs (v0)

This is the implementer spec, written after CONTRARIAN.md. It locks in defaults for the seven open questions and specifies programs, accounts, instructions, invariants, and the off-chain integration surface.

**Status**: pre-implementation. The Anchor workspace under `programs/` is scaffolded but not feature-complete. Use this doc + the scaffolded source as the starting point for the first Anchor sprint.

> **⚠ Read [AUDIT.md](./AUDIT.md) before writing any handler bodies.** Three independent reviews of this DESIGN + scaffold found 6 P0 (fund-loss) issues, 6 P1 issues, and 8 design-level questions that need team decisions. Mechanical fixes are applied in the scaffold; design questions (MPL Agent Registry vs SATI, Squads onboarding flow, Sybil mitigation, Kora self-host vs hosted) are listed in AUDIT.md "Open design questions" and must be resolved before coding handlers.

---

## 1. Decisions taken (the 7 open questions, defaulted)

| # | Question (from CONTRARIAN) | Default | Why |
|---|---|---|---|
| 1 | Agents OK as Squads vault holders? | **Yes — required for v1** | Squads SpendingLimit is the entire mandate primitive. Plain-EOA support deferred to v2 if demand exists. |
| 2 | Receivables ever from non-keyed sources? | **Yes — keep worker-attest fallback** | Legacy webhooks (Stripe, etc.) won't sign on-chain. Hybrid: ed25519 path for keyed sources, worker-write for the long tail. |
| 3 | Third-party readability of state? | **Yes — design for it** | Other lenders, explorers, and agent-tooling ecosystems should compose. Drives on-chain PDA model. |
| 4 | Audit budget? | **Tight — minimize on-chain code** | One audit pass on `credmesh-escrow` + `credmesh-reputation`. Drop the credit-oracle program; fold curve math into escrow. |
| 5 | Dashboard "single-process timeline" semantics? | **No — derived-view cache only** | SQLite stays as a cache for fast paginated reads; canonical source is on-chain events ingested via Helius webhooks. |
| 6 | Credit curve complexity (ML / multi-signal)? | **Tier curve only for v1** | One integer (`score_ema`) → max-credit USD via piecewise-linear curve stored on `Pool` PDA. ML-derived stays out of scope. |
| 7 | Solana Mobile / MWA agents? | **Not v1** | Browser/extension wallets only. Defer Mobile Wallet Adapter to v2. |

These defaults are revisable — call them out explicitly when revising.

## 2. Workspace layout

```
credmesh-solana/
├── DESIGN.md                          (this doc)
├── Anchor.toml                        anchor workspace config
├── Cargo.toml                         workspace root
├── programs/
│   ├── credmesh-escrow/               vault + advance + claim_and_settle
│   ├── credmesh-reputation/           8004-shape, CredMesh-owned (NOT 8004-solana)
│   └── credmesh-receivable-oracle/    worker-attested + ed25519-verified
├── ts/
│   ├── server/                        Hono + Helius + Kit + Codama clients
│   ├── dashboard/                     React 19 + Phantom Connect + ConnectorKit
│   └── mcp-server/                    HTTP-API wrapper, no chain code
├── tests/
│   ├── bankrun/                       fast unit/integration (anchor-bankrun)
│   ├── litesvm/                       property/fuzz tests
│   └── devnet/                        end-to-end with real USDC
└── research/                          existing research package
```

External dependencies (programs CredMesh **uses** but does not deploy):
- **Squads v4** (`SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf`) — agent vaults, SpendingLimit PDAs, governance multisig
- **Solana Agent Registry** — CredMesh agents register a Metaplex Core asset; we read its pubkey as our agent ID
- **SPL Token classic** — USDC vault; share-mint
- **ed25519 program** (`Ed25519SigVerify111111111111111111111111111`) — receivable attestation verification
- **Memo program** (`MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr`) — replay-nonce binding

## 3. `credmesh-escrow` program

### 3.1 Accounts (PDAs)

| PDA | Seeds | Size | Lifetime |
|---|---|---|---|
| `Pool` | `[b"pool", asset_mint]` | ~400 B | Permanent |
| `Advance` | `[b"advance", agent_pubkey, receivable_id]` | ~200 B | Init at request, close on settle/liquidate |
| `ConsumedPayment` | `[b"consumed", receivable_id]` | 16 B | Init at request, close on settle (rent → agent) |
| `ProtocolTreasury` | `[b"treasury"]` | ~80 B | Permanent |

### 3.2 `Pool` account fields

```rust
pub struct Pool {
    pub bump: u8,
    pub asset_mint: Pubkey,            // USDC mint
    pub usdc_vault: Pubkey,            // ATA owned by Pool PDA
    pub share_mint: Pubkey,            // SPL mint, mint_authority = Pool PDA
    pub governance: Pubkey,            // Squads vault address
    pub total_assets: u64,             // 6-decimal USDC, virtual-shares accounting
    pub total_shares: u64,             // share-mint supply
    pub deployed_amount: u64,          // sum of unsettled Advance.principal
    pub accrued_protocol_fees: u64,    // claimed via skim, not auto-flowed
    pub virtual_assets_offset: u64,    // hardcoded 1_000_000 (1 USDC) — first-depositor defense
    pub virtual_shares_offset: u64,    // hardcoded 1_000_000_000 (10**9)
    pub fee_curve: FeeCurve,           // 4-component dynamic pricing params
    pub max_advance_pct_bps: u16,      // 3000 = 30% of receivable
    pub max_advance_abs: u64,          // $100 = 100_000_000
    pub timelock_seconds: i64,         // governance delay
    pub pending_params: Option<PendingParams>,
    pub paused: bool,                  // governance can pause init/withdraw only — NEVER advance issuance
}

pub struct FeeCurve {
    pub utilization_kink_bps: u16,     // 8000 = 80% utilization
    pub base_rate_bps: u16,            // 200 = 2% floor
    pub kink_rate_bps: u16,            // rate at kink utilization
    pub max_rate_bps: u16,             // 2500 = 25% cap
    pub duration_per_day_bps: u16,     // additive duration component
    pub risk_premium_bps: u16,         // multiplier on score_ema
    pub pool_loss_surcharge_bps: u16,  // post-default penalty
}
```

### 3.3 `Advance` account fields

```rust
pub struct Advance {
    pub bump: u8,
    pub agent: Pubkey,                 // Solana Agent Registry asset pubkey
    pub receivable_id: [u8; 32],       // hash of (source, source_id, expiry)
    pub principal: u64,                // USDC issued
    pub fee_owed: u64,                 // computed at issue, fixed
    pub late_penalty_per_day: u64,     // 0 if not late
    pub issued_at: i64,                // unix seconds
    pub expires_at: i64,               // unix seconds; receivable expiry
    pub source_kind: u8,               // 0=worker, 1=ed25519-attested, 2=x402
    pub source_signer: Option<Pubkey>, // present iff source_kind != 0
    pub state: AdvanceState,           // Issued | Settled | Liquidated
}
```

### 3.4 Instructions

#### `init_pool(params)` — governance only

Signers: Squads vault. Inits Pool PDA, share mint, USDC vault ATA. Mints `virtual_shares_offset` to Pool PDA itself (irrecoverable; first-depositor defense).

#### `deposit(amount)` — anyone

```
shares_minted = (amount * (total_shares + virtual_shares_offset)) / (total_assets + virtual_assets_offset)
```

Updates `total_assets += amount`, mints `shares_minted` to LP's share ATA.

#### `withdraw(shares)` — share holder

```
assets_returned = (shares * (total_assets + virtual_assets_offset)) / (total_shares + virtual_shares_offset)
```

**Invariant**: `usdc_vault.amount >= assets_returned`. Otherwise `InsufficientIdleLiquidity` — the deployed capital is locked, this enforces idle-only withdrawals.

Burns `shares` from LP, transfers `assets_returned` USDC out, decrements `total_assets`.

#### `request_advance(receivable_id, amount, source_kind, attestation?)` — agent

Accounts:
- `agent` (signer) — agent's authority (Squads vault member or directly)
- `agent_asset` — Solana Agent Registry asset pubkey (read-only, validated)
- `agent_reputation_pda` — owned by `credmesh-reputation`, read via re-derive
- `receivable_pda` — owned by `credmesh-receivable-oracle`, read via re-derive
- `pool` (mut)
- `advance_pda` (mut, init)
- `consumed_pda` (mut, init) — replay protection; `init` semantics fail if exists
- `pool_usdc_vault` (mut)
- `agent_usdc_ata` (mut, init_if_needed)
- `usdc_mint`
- `token_program`, `system_program`, `rent`

Optional:
- `instructions_sysvar` — required if `source_kind = 1` (ed25519 verify pre-instruction)

Logic:
1. **Replay**: `consumed_pda` init succeeds iff this `receivable_id` has never been used.
2. **Receivable verification** by `source_kind`:
   - `0` (worker): re-derive `receivable_pda`, deserialize, check `last_updated_slot` recency.
   - `1` (ed25519): instruction-introspection on previous instruction; verify it called the ed25519 program with `(source_signer pubkey, expected message: receivable_id || agent || amount || expiry, signature)`.
   - `2` (x402): same as `1` but `source_signer` must be in the Pool's allowlist (CredMesh-curated facilitators).
3. **Credit check**: re-derive `agent_reputation_pda`. Read `score_ema` (u64 with `score_decimals`). Apply `Pool.fee_curve` tier curve to compute `max_credit`.
4. **Cap check**: `amount <= min(receivable_amount * max_advance_pct_bps / 10000, max_advance_abs, max_credit)`.
5. **Fee compute**: `fee_owed = price(amount, utilization_after, duration, risk)` — `pricing.ts` math, ported.
6. **Settle math**: `Pool.deployed_amount += principal`. Init `Advance` PDA. Transfer `principal` USDC from `pool_usdc_vault` to `agent_usdc_ata`.
7. Emit `AdvanceIssued` event.

#### `claim_and_settle(payment_proof)` — permissionless after `expires_at - 7 days`

Accounts:
- `cranker` (signer, fee payer)
- `advance_pda` (mut, close = agent on full settlement)
- `consumed_pda` (mut, close = agent)
- `pool` (mut)
- `pool_usdc_vault` (mut)
- `agent_usdc_ata` (mut) — receives net
- `protocol_treasury_ata` (mut) — receives 15%
- `payer_usdc_ata` (mut) — source of repayment, includes signer iff agent-cranked
- `usdc_mint`, `token_program`

Logic:
1. **Memo nonce check** (instruction-introspection): payment instruction earlier in tx must include the memo with `consumed_pda.nonce`.
2. **Compute waterfall**:
   ```
   late_days = max(0, now - expires_at) / 86400
   late_penalty = late_days * advance.late_penalty_per_day
   total_owed = principal + fee_owed + late_penalty
   protocol_cut = (fee_owed + late_penalty) * 1500 / 10000   // 15%
   lp_cut       = principal + (fee_owed + late_penalty) * 8500 / 10000  // 85% of fees + principal
   agent_net    = received - protocol_cut - lp_cut             // remainder to agent
   ```
3. **Three CPI'd `transfer_checked`** in this exact order, all in this single instruction:
   1. `protocol_cut` to `protocol_treasury_ata`
   2. `lp_cut` to `pool_usdc_vault`
   3. `agent_net` to `agent_usdc_ata`
4. **Pool update**: `deployed_amount -= principal`, `total_assets += (lp_cut - principal)` (the fee portion).
5. **Close** `advance_pda` and `consumed_pda`, send rent to `agent` (NOT cranker — neutralizes MEV cranking).
6. Emit `AdvanceSettled` event.

#### `liquidate(advance_id)` — anyone after `expires_at + 14 days`

Marks default. `Pool.deployed_amount -= principal`, `total_assets -= principal` (LPs eat the loss pro-rata via share-price drop). Optionally apply `pool_loss_surcharge_bps` to fee curve for next N advances.

#### `propose_params(new_params)` / `execute_params()` — governance

Two-step timelock. `propose_params` writes `pending_params` with `execute_after = now + timelock_seconds`. `execute_params` requires `now >= execute_after` and applies. Governance is a Squads vault.

#### `skim_protocol_fees(amount)` — governance

Withdraws accumulated `Pool.accrued_protocol_fees` to a governance-specified ATA. Capped at the actual accrued amount.

### 3.5 Invariants (enforced by code, checked in tests)

- `usdc_vault.amount + deployed_amount >= total_assets` (with virtual-shares math, shares can never out-redeem real assets)
- `withdraw(s)` always satisfies `usdc_vault.amount >= preview_redeem(s)` or fails atomically
- `request_advance` has no pause path. Advance issuance is never gated by governance.
- `consumed_pda` is **permanent** (audit P0-5). `init` failure on duplicate `receivable_id` is the sole replay-protection mechanism. Closing it would allow close-then-reinit replay in a single tx.
- Waterfall transfers in `claim_and_settle` always sum to `received` (no rounding drift; remainder rounds to agent)
- `Advance` survives `liquidate` with `state = Liquidated` for audit trail (audit AM-7).
- **Early-liquidation lever** (audit AM-8): a permissionless cranker MAY call `liquidate` at `expires_at + 14 days`. The off-chain server SHOULD prioritize `claim_and_settle` whenever a payment is observed, even partial, to prevent unnecessary defaults.

### 3.6 Errors

```rust
#[error_code]
pub enum CredmeshError {
    #[msg("Insufficient idle liquidity in pool")]
    InsufficientIdleLiquidity = 6000,
    #[msg("Receivable expired or not yet valid")]
    ReceivableExpired,
    #[msg("Advance amount exceeds receivable cap")]
    AdvanceExceedsCap,
    #[msg("Advance amount exceeds reputation-derived credit limit")]
    AdvanceExceedsCredit,
    #[msg("Receivable digest does not match expected")]
    DigestMismatch,
    #[msg("Receivable PDA stale")]
    ReceivableStale,
    #[msg("ed25519 verification missing or wrong format")]
    Ed25519Missing,
    #[msg("ed25519 signer not in allowlist")]
    Ed25519SignerUnknown,
    #[msg("Memo nonce does not match consumed PDA")]
    MemoNonceMismatch,
    #[msg("Advance not yet settleable (claim window)")]
    NotSettleable,
    #[msg("Advance not yet liquidatable (grace period)")]
    NotLiquidatable,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Governance signer required")]
    GovernanceRequired,
    #[msg("Pending params not yet executable")]
    PendingParamsNotReady,
    #[msg("Replay detected: consumed PDA already exists")]
    ReplayDetected,
}
```

### 3.7 Events

```rust
#[event] pub struct PoolInitialized { pool: Pubkey, asset_mint: Pubkey, share_mint: Pubkey, governance: Pubkey }
#[event] pub struct Deposited       { pool: Pubkey, lp: Pubkey, amount: u64, shares_minted: u64 }
#[event] pub struct Withdrew        { pool: Pubkey, lp: Pubkey, shares_burned: u64, assets_returned: u64 }
#[event] pub struct AdvanceIssued   { pool: Pubkey, agent: Pubkey, advance: Pubkey, principal: u64, fee_owed: u64, expires_at: i64, source_kind: u8 }
#[event] pub struct AdvanceSettled  { pool: Pubkey, agent: Pubkey, advance: Pubkey, principal: u64, lp_cut: u64, protocol_cut: u64, agent_net: u64, late_days: u32 }
#[event] pub struct AdvanceLiquidated { pool: Pubkey, agent: Pubkey, advance: Pubkey, loss: u64 }
#[event] pub struct ParamsProposed  { pool: Pubkey, execute_after: i64 }
#[event] pub struct ParamsExecuted  { pool: Pubkey }
```

## 4. `credmesh-reputation` program

CredMesh-owned (per REVIEW.md disagreement #1 — fork the 8004 shape, don't depend on 8004-solana). Same data shape so any 8004-shaped indexer can read it.

### Accounts

```rust
pub struct AgentReputation {
    pub bump: u8,
    pub agent_asset: Pubkey,           // Solana Agent Registry asset
    pub feedback_count: u64,
    pub feedback_digest: [u8; 32],     // rolling keccak: digest = keccak(prev || event_hash)
    pub score_ema: u64,                // 18-decimal EMA over last N events
    pub score_decimals: u8,            // 18
    pub default_count: u32,
    pub last_event_slot: u64,
}
```

### Instructions

- `init_reputation(agent_asset)` — anyone
- `give_feedback(agent_asset, score, value, value_decimals, reason_code, feedback_uri, feedback_hash, job_id, attestor_signature?)` — permissionless write; consumer-side filter decides which writers count
- `append_response(...)` — agent appends a public response to a feedback
- `revoke_feedback(...)` — original signer can revoke

### Read pattern (from `credmesh-escrow`)

`agent_reputation_pda` is passed in `request_advance` accounts. Escrow re-derives the PDA address from `agent_asset` and asserts equality. Deserializes `AgentReputation`, reads `score_ema` and `default_count`. No CPI.

## 5. `credmesh-receivable-oracle` program

### Accounts

```rust
pub struct Receivable {
    pub bump: u8,
    pub agent: Pubkey,
    pub source_id: [u8; 32],           // payer-defined identifier
    pub source_kind: u8,               // 0=worker, 1=ed25519-attested, 2=x402
    pub source_signer: Option<Pubkey>, // present for kind 1/2
    pub amount: u64,                   // 6-decimal USDC
    pub expires_at: i64,
    pub last_updated_slot: u64,
    pub authority: Pubkey,             // worker or signer
}

pub struct AllowedSigner {             // for x402 facilitators / known exchanges
    pub bump: u8,
    pub signer: Pubkey,
    pub kind: u8,                       // 1=exchange, 2=x402_facilitator
    pub max_per_receivable: u64,        // per-tx cap (mitigates compromised key)
    pub max_per_period: u64,
    pub period_seconds: i64,
    pub period_start: i64,
    pub period_used: u64,
}
```

### Instructions

- `init_oracle(governance)` — one-time
- `worker_update_receivable(agent, source_id, amount, expires_at)` — worker authority writes; bounded by per-tx cap
- `ed25519_record_receivable(agent, source_id, amount, expires_at)` — caller passes, ed25519 verify in prior instruction; oracle just persists
- `add_allowed_signer(signer, kind, caps)` / `remove_allowed_signer` — governance only

The escrow's `request_advance` may bypass this program for `source_kind=1` and verify the ed25519 directly without reading a Receivable PDA — that's the cleanest path for x402. The PDA storage is for `source_kind=0` (worker) so the receivable is durable for staleness checks.

## 6. Off-chain server changes

The Hono backend stays. Diff vs current:

- **`src/chain.ts`** → split into `src/svm.ts` (Solana via `@solana/kit` + Codama-generated `credmesh-escrow` client + Helius SDK) and `src/evm.ts` (existing viem code, unchanged for the legacy Base deployment). `chains.ts` gets `kind: "evm" | "svm"` discriminant.
- **Auth middleware**: SIWS verifier using `tweetnacl`. Headers: `X-Agent-Address` (base58), `X-Agent-Signature` (base58), `X-Agent-Timestamp` (ISO), `X-Agent-Cluster` (`mainnet-beta`/`devnet`), optional `X-Agent-Nonce`.
- **`POST /agents/:address/advance`** → calls `buildRequestAdvanceTx()` which:
  1. Fetches the agent's Solana Agent Registry asset (DAS lookup).
  2. Derives `Advance`, `Consumed`, `Reputation`, `Receivable` PDAs.
  3. Includes ed25519 verify ix if `source_kind=1`.
  4. Uses Helius `getPriorityFeeEstimate` with the writable accounts.
  5. Returns `{ tx: base64, lastValidBlockHeight, nonce }`.
- **Trustless mode collapsed into the same path**: agent always signs `request_advance`, Kora always pays SOL. There is no operator-mode-special-case anymore.
- **Webhook ingest**: new route `POST /webhooks/helius` with `X-Helius-Auth` check. Ingests `AdvanceIssued`, `AdvanceSettled`, `Deposited`, etc. Updates the SQLite derived-view cache.
- **Replay map deleted**: `state.consumedPayments` is gone. Replay protection is the on-chain `Consumed` PDA.
- **Pricing**: `pricing.ts` math runs server-side for quoting, but the same parameters are stored on-chain in `Pool.fee_curve` so the program enforces them. Server quote and on-chain quote must match (asserted in tests).

## 7. Test plan

| Layer | Tool | What we test |
|---|---|---|
| Unit | `anchor-bankrun` | Each instruction in isolation: math, account constraints, error paths |
| Property | `litesvm` + `proptest` | Waterfall sum invariant; share-price monotonicity post-fee; no-double-spend on `Consumed` PDA; first-depositor inflation cost ≥ 10⁶× attacker profit |
| Integration | `anchor test` (localnet) | Multi-program flows: register reputation → request_advance → claim_and_settle |
| End-to-end | devnet | Real Circle USDC, real Squads vault for the agent, real Helius webhooks, real ed25519 verification |
| Mainnet staging | mainnet-beta | $10–$100 cap; real LPs; one full cycle |

## 8. Phased implementation order

1. **`credmesh-escrow` minimal**: `init_pool`, `deposit`, `withdraw`, `request_advance` (worker-attested only), `claim_and_settle`, `liquidate`. No governance proposals yet — `governance` field is just authority; params are constructor-only.
2. **`credmesh-reputation`**: `init_reputation`, `give_feedback` (permissionless), reading from escrow.
3. **Worker auth + tx-builder + webhook ingest**: SIWS, `buildRequestAdvanceTx`, Helius webhook handler.
4. **Squads vault integration**: agent registration creates a Squads vault for the agent; mandates become Squads SpendingLimit PDAs.
5. **ed25519 receivable verification**: x402 facilitator allowlist + ed25519 path in `request_advance`.
6. **Governance + timelock**: `propose_params`, `execute_params`, Squads-as-governance.
7. **Dashboard fork**: replace wagmi/viem with Phantom Connect + ConnectorKit.
8. **CCTP v2 fallback**: cross-chain agent flows (Solana credit pays Base x402 server).

Each phase is independently shippable. Audit happens after phase 6.

## 8.5 v2 sharding plan (audit AM-9)

The single `Pool` PDA write-locks every advance issuance, settle, and liquidate. Realistic v1 ceiling is ~30–80 advances/sec — well above expected demand. v2 sharding is straightforward: change the Pool seed from `[POOL_SEED, asset_mint]` to `[POOL_SEED, asset_mint, tier_id]` so risk-band pools parallelize on Sealevel. Document now, ship at the v2 boundary.

## 9. What is explicitly **not** in v1

- ML-derived credit curves (defaulted-out)
- Mobile Wallet Adapter (defaulted-out)
- Hyperliquid Lazer publisher (Lazer-feed path not yet available)
- Light Protocol compressed PDAs (revisit at scale; classic Anchor PDAs for v1)
- Plain-EOA agent support (Squads-only for v1)
- Multi-asset pools (USDC only)
- Per-instruction-type timelock granularity (Squads multisig has global timelock — accept for v1)
- Token-2022 USDC migration handling (Circle hasn't moved; revisit when they do)
- Embedded-wallet (Phantom Portal) auth — SIWS detached signing isn't supported (per audit integration review).
- Permissionless `claim_and_settle` cranking — v1 requires `cranker == advance.agent` (audit P0-3/P0-4). Permissionless settle requires a future payer-pre-authorized signing pattern.

## 10. Threat model and key topology (audit Q7)

CredMesh requires **three logically separate keys**, never the same:

1. **Fee-payer key** (Kora signer or hosted facilitator). Hot, low value, rotates aggressively. Pays SOL for `request_advance` and `claim_and_settle` when the agent flow goes through CredMesh's relayer. Compromise = griefing only (sponsor txs that fail).
2. **Oracle worker authority** (`OracleConfig.worker_authority`). Writes `Receivable` PDAs for `source_kind = Worker`. **Highest-value key in the system.** Compromise = inflate receivables, request advance, drain LP capital. Mitigations:
   - Per-tx cap (`worker_max_per_tx`) and per-period cap (`worker_max_per_period`) on `OracleConfig`.
   - Rotation flow via `governance` (Squads multisig).
   - **Must not be co-located with the fee-payer key.**
3. **Reputation provider key** (only if Q5 = "yes/v1.5"). Signs `ReputationScoreV3` SAS attestations. Compromise = inflate reputation; capped by Sybil mitigation (Q4).

**Governance** is a Squads v4 multisig PDA. It is not a Signer — it executes on-chain by CPI from the Squads program. The `governance: Signer` constraint on `propose_params` / `skim_protocol_fees` / `add_allowed_signer` is currently a placeholder until Q3 is resolved (3-tx onboarding flow vs Controlled-Multisig).

**Key rotation**: the Squads multisig holds upgrade authority on all three programs and the `OracleConfig.worker_authority` setter. Rotation = a Squads-approved tx that updates the relevant authority field; takes the configured timelock_seconds to execute.
