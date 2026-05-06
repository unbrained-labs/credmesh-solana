# BRUTAL TRUTH — what's on Solana isn't a port of the EVM lane

**Status:** acknowledged drift, refactor in progress on `dev`.
**Authored:** 2026-05-06.
**Scope:** what was fucked up, what the actual vision is, what we're doing now.

This is not a contrarian-style design doc. It's a postmortem of a mistake plus a plan to fix it.

---

## The vision

CredMesh is supposed to be a **credit card / factoring / credit-line for autonomous agents** working in agent-to-agent marketplaces. No human in the loop. Standing credit limit derived from on-chain reputation. Instant deterministic underwriting. Automatic repayment from job revenue. Permissionless across the board.

In one line, from the EVM repo's own README:

> Agent wins a $100 job → requests a $15 advance → gets funded → completes work → payout repays the advance → credit limit improves

That's what the user signed up to build. That's the EVM lane, alive and shippable today.

## What got built on Solana instead

A different protocol. **Per-receivable secured installment lender** with heavy preregistration, structural human-approval gates via Squads, MPL-Core-required identity, and a receivable abstraction that requires non-agent participation for every advance.

This wasn't introduced by the recent permissionless-settle work on `dev`. The drift was **baked into the original Solana design** (DECISIONS.md Q1, Q3, Q4) before any of this branch's commits. The recent work on `dev` (split + Mode A/B settlement) made one piece — settlement — actually permissionless. But it didn't address the structural drift in identity, onboarding, credit-line semantics, or the receivable model.

## Concrete drift list (EVM vs Solana, the real code)

### 1. Credit line vs per-tier ceiling

EVM (`packages/credit-worker/src/credit.ts:24-56`):
```js
score        = repaidAdvances*5 + successfulJobs*1.6 + attestationCount*0.7
             + cooperationSuccessCount*1.5 + trustScore*0.08 + averageCompletedPayout*0.02
             - failedJobs*6 - defaultedAdvances*25 - outstandingBalance*0.2
creditLimit  = score*8 + repayRate*120 + completionRate*80      // up to $1000
availableCredit = creditLimit - outstandingBalance
```

A standing credit line. Rolling exposure. Repayment improves the limit.

Solana (`programs/credmesh-escrow/src/pricing.rs::credit_from_score_ema`):
```rust
match score_int {
    0..=20 => 0u64,
    21..=49 => 10_000_000,   // $10
    50..=69 => 25_000_000,   // $25
    70..=84 => 100_000_000,  // $100
    85..=94 => 200_000_000,  // $200
    _ => 250_000_000,        // $250
}
```

Per-tier ceiling. No `outstanding_balance` field on AgentReputation. No rolling exposure tracking. No way for an agent to query "what's my available credit right now."

### 2. Onboarding cost and gates

EVM: one HTTP call.
```
POST /agents/register { address, name, trustScore, attestationCount, successfulJobs, ... }
```
Optional ERC-8004 registration on-chain. No mandatory ecosystem dependencies.

Solana: five separate registrations, each with rent + protocol coordination:
- MPL Core asset mint (~0.009 SOL, ~$1.50)
- AgentIdentity PDA on MPL Agent Registry
- DelegateExecutionV1 record on MPL Agent Tools (for headless hot-key)
- AgentReputation PDA on credmesh-reputation
- Squads v4 vault (~0.013 SOL + 0.1 SOL platform fee = ~$22) with **CredMesh as `configAuthority` and a bilateral off-ramp** per DECISIONS Q3

The Squads bilateral off-ramp is **literally a human approval gate**. From DECISIONS.md Q3 itself:

> "the off-ramp is *bilateral*: agent requests → CredMesh governance executes a vault transaction setting `config_authority = Pubkey::default()`. **This is not a one-click unilateral exit.**"

For an agent to onboard: CredMesh has to execute a vault tx adding a SpendingLimit. For an agent to leave: CredMesh has to sign a config-authority release. Both directions are gated by CredMesh's signature. **This is not autonomous.**

### 3. The receivable abstraction

EVM: marketplace posts the job. That IS the receivable.
```
POST /marketplace/jobs { agentAddress, payer, expectedPayout, durationHours, category }
```

Solana: the receivable PDA on `credmesh-receivable-oracle` is created by either:
- **Worker path:** CredMesh's oracle worker (a CredMesh-controlled key) writes the Receivable PDA
- **Ed25519 path:** the **payer** signs a 96-byte canonical message (`receivable_id || agent || amount || expires_at || nonce`) which the agent submits as a preceding ix

Neither is agent-autonomous. The "Worker" path means CredMesh participates in every advance. The "Ed25519" path means the payer participates in every advance (must sign with a key registered as an `AllowedSigner` PDA).

For an A2A loop where Agent A wants to hire Agent B: someone non-agent has to attest to the job's existence on-chain. **The autonomous case is structurally impossible.**

### 4. `request_advance` complexity

EVM contract: 3 arguments.
```solidity
TrustlessEscrow.requestAdvance(address oracle, bytes32 receivableId, uint256 requestedAmount)
```

Solana ix: requires 15+ Anchor accounts:
- agent (Signer)
- agent_asset (MPL Core, owner-checked)
- agent_identity (MPL Agent Registry PDA)
- agent_reputation_pda (credmesh-reputation typed Account)
- receivable_pda (typed Account, optional based on source_kind)
- executive_profile + execution_delegate_record (MPL Agent Tools, optional)
- pool, advance, consumed (3 PDAs)
- pool_usdc_vault, agent_usdc_ata
- usdc_mint, instructions_sysvar
- token_program, associated_token_program, system_program

Most of these have to be pre-derived/pre-existing. There's no way to "just ask for $15."

### 5. Settlement automation

EVM: `settle(advanceId, payoutAmount)` — anyone calls. Pulls repayment from `msg.sender` via standard ERC-20 `transferFrom`. The marketplace can call this directly with the job's payout, repaying the advance with zero agent involvement. Repayment is automatic.

Solana on `dev` (the work I just did): two-mode dispatch.
- Mode A: agent self-cranks (legacy). Agent must come back online within 7-day window.
- Mode B: any cranker, but pulls from `agent_usdc_ata` via SPL delegate granted at request_advance time. Requires the receivable's payment to land in the agent's ATA first.

Neither mode supports the EVM "marketplace pays directly with its USDC, agent never involved in settlement" flow. **There's no `claim_and_settle_from_caller` mode where the cranker provides the funds.**

### 6. Issue #40 — governance is uncallable today

`propose_params` and `skim_protocol_fees` declare `pub governance: Signer<'info>`. Squads vault PDAs cannot be Signers (they sign via CPI). The governance loop is uncallable from a real Squads vault. Currently the only way to update fee curves is to upgrade the program (which is itself behind the deployer wallet, not Squads, until V1_ACCEPTANCE gate #4 is met).

### 7. `transfer_checked` vs bare `transfer` — CLAUDE.md hard-rule violation

CLAUDE.md line 91: *"Don't use bare `transfer` — Token-2022 forward-compat requires `transfer_checked`."*
Reality: `grep -rn "transfer_checked" programs/credmesh-escrow/src/` returns 0 hits. The codebase uses `token::transfer` everywhere. Pre-existing, but flagged repeatedly across reviews and never fixed.

### 8. The outreach agent doesn't have a Solana scanner

`packages/outreach-agent/src/scanners/` has `hyperliquid.ts` only. No `solana.ts`. The "scan underperforming vaults → pitch CredMesh as higher-yield deposit target" liquidity-sourcing loop doesn't exist for Solana.

---

## Why this is fixable

The escrow's **waterfall math, virtual-shares, replay defenses (P0-5 ConsumedPayment permanence, memo-nonce binding, ed25519 message offset asserts), MEV-neutral close=agent rent routing, audit-clean ATA-substitution defenses, and the new permissionless Mode A/B settlement** all carry over. Those are real and good.

The drift is in five places, all addressable without touching the audit-clean primitives:

1. **`AgentReputation` schema** needs `credit_limit_atoms`, `outstanding_balance_atoms`, `successful_jobs`, `failed_jobs`, `repaid_advances`, `defaulted_advances`, `cooperation_success_count`, `attestation_count`, `trust_score`, `average_completed_payout`. Score formula and limit formula port byte-for-byte from `credit.ts:24-56`.
2. **`register_agent` ix** on credmesh-reputation: one-tx onboarding, agent's keypair as signer. **No MPL Core, no Squads vault required.** Optional attestation upgrades come later.
3. **`register_job` ix** on credmesh-receivable-oracle: permissionless, claim-type-based ratio (10% / 20% / 30% per `protocol-spec/index.js`). The marketplace OR the agent posts. Worker path becomes one option among three, not the only path.
4. **`claim_and_settle` Mode 3** (NEW): cranker provides USDC from their own ATA. `payer_usdc_ata.token::authority = cranker`. This is the EVM `settle(advanceId, payout)` semantics — the marketplace calls with its own funds and the agent is never involved in settlement.
5. **Issue #40 fix**: `propose_params` / `skim_protocol_fees` accept `governance: UncheckedAccount<'info>` address-pinned to `pool.governance`, with a `verify_squads_cpi` helper in `credmesh-shared` that introspects the call stack.

Plus #8 — Solana scanner in the outreach agent + `outreach` block exposed by the Solana worker's agent card.

The Squads-as-configAuthority becomes **opt-in for agents who want a multisig treasury**. The protocol no longer requires it. MPL Core also becomes opt-in. Agents can be raw keypairs.

---

## What this branch (`dev`) is now doing

Path A — faithful EVM port. No new branches. Sequential commits on `dev`. Goal: parity by the end of the work, no carve-outs.

Order of operations:

1. ✅ This doc.
2. **AgentReputation schema port** — add the 9 fields, write `register_agent`, port the score + credit-limit formulas.
3. **Escrow integration** — `request_advance` enforces against `credit_limit - outstanding_balance` (not the tier curve). `claim_and_settle` and `liquidate` update outstanding_balance via CPI to credmesh-reputation. Drop the MPL Core constraint to optional.
4. **Marketplace primitive** — `register_job` ix on credmesh-receivable-oracle, permissionless, claim-type-based.
5. **Mode 3 settlement** — cranker funds the repayment from their own ATA. The marketplace's automatic-repayment path.
6. **Issue #40 fix** — Squads CPI verification helper, `governance: UncheckedAccount` shift.
7. **`transfer_checked` migration** — close the CLAUDE.md hard-rule violation.
8. **Outreach agent — Solana scanner** — `packages/outreach-agent/src/scanners/solana.ts` (in the EVM repo, since that's where the outreach agent lives) + `outreach` agent-card block exposed by the Solana worker.
9. **Tests** — bankrun specs for the new flows; pure-math suites for the credit-limit formula matching EVM golden vectors.
10. **Doc cleanup** — DECISIONS.md Q3 amended (Squads opt-in), DECISIONS.md Q1 amended (MPL Core optional), AUDIT.md updated, V1_ACCEPTANCE.md updated, CLAUDE.md "What NOT to do" updated.

Each step lands as its own commit on `dev` so the diff is reviewable.

## Investigation note — issue #15 IDL extraction (2026-05-06)

Spent a session attempting reproduction. The actual error in
anchor-syn 0.30.1 is at `src/idl/external.rs:21`:

```rust
std::env::var("ANCHOR_IDL_BUILD_PROGRAM_PATH").expect("Failed to get program path");
```

This is **set by the `anchor build` CLI when it invokes the IDL pass** —
NOT by raw `cargo build --features idl-build`. So:

- Local `cargo build --features idl-build` will always panic with
  "Failed to get program path" because the env var isn't exported.
- The original issue #15 is about a DIFFERENT failure mode: the IDL
  pass not resolving the `AssociatedToken` type even when invoked
  correctly via `anchor build`. The escrow already has a workaround
  comment about this in `instructions/request_advance.rs`.
- Reproducing the actual issue #15 needs the Anchor CLI + the Docker
  recipe in DEPLOYMENT.md § Build environment (Docker).

**Status:** deferred. Needs a session with the Docker pinning + Anchor
CLI installed to actually reproduce + fix. Not blocking the on-chain
program correctness — every program builds and runs; the gap is the
TS-side typed-client generation.

## What this is NOT

- A new branch or fork. Everything goes on `dev`.
- A "let's keep both designs" compromise. We're picking the EVM model. The previous Solana-specific design choices that conflict with it (Squads-as-configAuthority default, MPL Core mandatory, receivable model that requires non-agent participation) are getting reversed or made opt-in.
- A scope-creep into liquidity sourcing as a new feature. The outreach-agent integration is just exposing the right metadata so the existing EVM-side outreach pipeline can pitch the Solana vault. The Solana program itself doesn't change for this.
- A claim that the work already done on `dev` is wasted. The split, the permissionless-settle Mode A/B, the simplify fixes — all carry over. We just add the missing layers above them.

---

## Pivot to EVM-as-source-of-truth (2026-05-06)

After the Path A port landed (register_agent / register_job / Mode 3 /
Squads CPI gate / transfer_checked / outreach card), the user pushed back
on the underlying premise: porting reputation + identity + governance to
Solana **duplicates the same primitives that already work on EVM**, and
keeps the two lanes drift-prone forever. Quote: *"can't we just use the
existing attestation system although is not Solana native? so the
protocol lives in Solana but the attestation+governance lives somewhere
else? duplicating governance doesn't sound like a good idea to me."*

That reframe is correct. The right model is:

- **EVM is the single source of truth** for identity, reputation,
  governance, and the attestor whitelist policy.
- **Solana is a credit-issuance + settlement venue** that consumes
  EVM-attested credit limits via short-TTL ed25519 signatures from a
  whitelisted bridge signer.

### What got deleted

- `programs/credmesh-reputation/` — entire program (~1100 LoC). Score
  formula, AgentReputation PDA, give_feedback, register_agent,
  update_agent_attestations — all gone. EVM owns this.
- `crates/credmesh-shared/src/mpl_identity.rs` + the MPL Core / MPL
  Agent Registry / MPL Agent Tools program-ID constants. Identity is
  the agent's keypair on Solana; identity registration lives on EVM.
- `tests/bankrun/` — 15 files of `expect(true).to.be.true` placeholder
  specs gated on issue #15. Replaced by the bridge typecheck and the
  pure-math suites.
- `ts/dashboard/` — the LP / Agent / Governance views that mocked
  on-chain state.
- `SourceKind` / `claim_ratio_bps` / `mpl_core_asset` / `mpl_delegate_record`
  modules.
- `ts/server` stub routes (`/agents/:address`, `/agents/:address/advance`,
  `/webhooks/helius`).

### What got renamed / rewritten

- `programs/credmesh-receivable-oracle/` → `programs/credmesh-attestor-registry/`.
  Receivables-as-PDAs are no longer a primitive. The program now just
  holds an `AllowedSigner` PDA whitelist with kind tags
  (`AttestorKind::CreditBridge` today).
- `programs/credmesh-escrow/src/instructions/request_advance.rs` —
  rewritten. Drops agent_asset / agent_identity / agent_reputation_pda /
  receivable_pda / executive_profile / execution_delegate_record. Adds
  `allowed_signer: Account<AllowedSigner>`. Handler verifies the prior
  ed25519 ix, validates the signer is whitelisted with kind=CreditBridge,
  decodes the canonical 128-byte message, checks freshness (≤ 15 min),
  agent + pool match, version = 1, and underwrites against
  `attested_credit_limit − attested_outstanding`.
- `programs/credmesh-escrow/src/instructions/claim_and_settle.rs` —
  simplified to single-mode (agent self-settles). The Mode A/B/3
  permissionless dispatch is reverted: in the bridge model, attestations
  are short-TTL and the agent is reachable to self-settle within the
  receivable window.
- `Advance.attestor: Pubkey` field added (audit trail for which bridge
  signer underwrote each advance).

### What got added

- `crates/credmesh-shared/src/lib.rs::ed25519_credit_message` —
  canonical 128-byte format with strict offsets:
  `[agent(32) | pool(32) | credit_limit(u64 LE) | outstanding(u64 LE) |
  expires_at(i64 LE) | attested_at(i64 LE) | nonce(16) |
  chain_id(u64 LE) | version(u64 LE)]`.
  Constants: `MAX_ATTESTATION_AGE_SECONDS = 15 * 60`,
  `CHAIN_ID_MAINNET = 1`, `CHAIN_ID_DEVNET = 2`, `VERSION = 1`.
- `ts/bridge/` — off-chain HTTP service. Reads EVM
  `ReputationCreditOracle.maxExposure(agent)` and
  `TrustlessEscrow.exposure(agent)` via viem, encodes the 128-byte
  message, signs with tweetnacl, returns `{message_b64,
  signature_b64, signer_pubkey_b58, expires_at, attested_at,
  credit_limit_atoms, outstanding_atoms}`. Refuses to start with any
  required env var missing.

### Why this is better than the literal port

1. **One reputation system, no drift.** Score formula, attestor
   whitelist policy, AgentRecord lifecycle — all live in one place.
   Solana-side bugs can't desync from EVM-side state.
2. **Three-key topology collapses to one trust root.** No more
   `oracle_worker_authority` + `reputation_writer_authority` + fee-payer
   triple. The two on-chain trust roots are the bridge ed25519 signer
   whitelist (governance-revocable) and Squads governance over
   `Pool.governance` + the registry. Compromise of a bridge key is
   bounded by the 15-min TTL on each attestation.
3. **No magic-creature trust models.** The previous design had a
   `worker_authority` writing receivables and a `reputation_writer_authority`
   writing scores — single keys pretending to be oracles. The bridge
   replaces both with a verifiable cross-chain attestation that's
   replayable from EVM state.
4. **Onboarding cost drops to zero on Solana.** Agents register on EVM
   (one HTTP call, EVM lane already supports). Solana sees them when
   they request their first attestation. No MPL Core mint, no Squads
   vault, no Solana-side reputation init.

### What's still pending (not blocking v1)

- **Solana event tail in `ts/bridge`** — subscribes to escrow program
  logs and replays AdvanceIssued/AdvanceSettled/AdvanceLiquidated to
  EVM AgentRecord. The replay endpoint shape is being finalized in the
  EVM repo. Until then, EVM `outstanding` reads from
  `TrustlessEscrow.exposure(agent)` cover the EVM-issued advances;
  Solana-issued advances are tracked in the bridge's in-memory index.
  Documented in `ts/bridge/README.md`.

### What this pivot is NOT

- **A retreat from autonomous-agent ergonomics.** Agents still onboard
  with one HTTP call (now to EVM, where it already worked). The
  Solana side just doesn't duplicate the same registration flow.
- **A trust assumption upgrade for the bridge.** The 15-min TTL +
  governance-revocable whitelist + multiple-signer-redundancy posture
  bounds the worst-case blast radius. Compare: the previous
  `worker_authority` had no TTL and a long-lived single-key signing
  surface.
- **A regression on the v1 audit posture.** The escrow's waterfall
  math, virtual-shares, ConsumedPayment permanence, memo-nonce binding,
  ed25519 offset asserts, MEV-neutral close=agent rent routing, and
  ATA-substitution defenses all carry over byte-for-byte. The pivot
  removes surfaces; it doesn't relax any.
