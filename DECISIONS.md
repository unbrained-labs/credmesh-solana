# DECISIONS — Resolutions for AUDIT.md open design questions

The user delegated the call on Q1, Q3, Q4, Q5, Q6. Decisions below are made by the model from the context of the four research docs + REVIEW + CONTRARIAN + the three audit reviews. Reasoning is included so the team can override any individual call without losing the surrounding context.

These choices unblock handler-body implementation. Where the decision required code changes, the changes are in the same commit.

---

## Pivot 2026-05-06 — EVM is the single source of truth (Q14)

The Path A faithful EVM port (Q3 amended, register_agent on Solana, Mode 3
settlement) was the right diagnosis but the wrong remedy. The actual fix is
**not to port reputation/identity/governance to Solana, but to consume the
EVM lane's existing primitives via a short-TTL ed25519 attestation bridge.**

The full rationale lives in `BRUTAL-TRUTH-EVM-PARITY-DRIFT.md` §
"Pivot to EVM-as-source-of-truth". Material consequences for prior
decisions:

- **Q1 (MPL Agent Registry)** — DEPRECATED. Identity is the agent's
  Solana keypair; identity registration lives on EVM. MPL Core, MPL
  Agent Registry, and MPL Agent Tools are NOT used. Constants removed
  from `credmesh-shared`.
- **Q3 (Squads onboarding)** — STILL AMENDED OPT-IN. Squads-as-
  configAuthority remains opt-in for agents who want it; default agent
  is a raw keypair. Squads governance over `Pool.governance` + the
  attestor registry stays.
- **Q4 (reputation Sybil mitigation)** — DEPRECATED on Solana.
  Reputation is EVM-only. The `reputation_writer_authority` field is
  gone. Score derivation, attestor whitelist policy, and AgentRecord
  lifecycle all live on EVM.
- **Q5 (multi-issuer SAS attestations)** — DEFERRED. SAS write-along
  no longer applies on Solana since reputation isn't on Solana.
- **Q9 / Q10 (permissionless settle, three-mode dispatch)** — REVERTED.
  `claim_and_settle` is single-mode (agent self-settles). The bridge
  model means attestations are short-TTL and online; the agent is
  reachable to self-settle within the receivable window. The SPL
  `Approve` delegate CPI is removed from `request_advance`.
- **Q11 (`register_job` permissionless marketplace primitive)** —
  DEPRECATED. No receivable-as-PDA primitive on Solana. Marketplaces
  attest to job existence on EVM (where `IdentityRegistry` +
  `ReputationCreditOracle` already account for them).
- **Q12 (Squads CPI verification on governance ixs)** — STILL
  IN EFFECT. Both `credmesh-escrow::propose_params` /
  `skim_protocol_fees` and `credmesh-attestor-registry::*` ixs use
  `require_squads_governance_cpi`.
- **Q13 (cross-lane outreach loop)** — STILL IN EFFECT but cosmetic.
  `ts/server` exposes `/.well-known/agent.json` with the outreach block.

### Q14 — Bridge attestation surface

**Decision**: a Solana program (`credmesh-attestor-registry`) holds a
governance-controlled `AllowedSigner` PDA whitelist with kind tags. An
off-chain bridge service (`ts/bridge`) reads the EVM lane, signs a
canonical 128-byte `ed25519_credit_message`, and returns it to the
agent. The agent submits a Solana tx with `[ed25519_verify(...),
request_advance(...)]`. The Solana handler verifies the prior ix is
ed25519, the signer is registered with `kind = AttestorKind::CreditBridge`,
the message offsets/version are exact, freshness ≤ 15 min, agent + pool
match, and `chain_id` matches the deploy.

**Why ed25519 over a custom signature scheme**: Solana ships a native
ed25519 precompile (the ed25519 program) that verifies sigs at < 1500
CU. We never roll our own crypto — we just check that the prior ix was
this precompile and that its offsets line up with our message.

**Why a 15-min TTL**: bounds the worst-case blast radius if a bridge
key is compromised. 15 min is short enough that an attacker can't
sustain fraudulent issuance after detection + revocation, and long
enough that an honest agent's tx confirmation window doesn't race.

**Why kind tags on AllowedSigner**: forward-compat. Today the only
kind is `CreditBridge`. Future kinds (e.g. `OutreachBridge`,
`SettlementCoordinator`) can be added without changing the registry's
storage layout — handlers filter by kind in their account constraints.

**Why any-valid-sig instead of quorum**: simpler v1, redundancy against
single-bridge-instance downtime. Multiple bridge signers can be
whitelisted concurrently. Quorum-required is a v1.5 hardening.

**Risk**: a compromised bridge key can issue fraudulent attestations
within the 15-min window. Mitigations: (1) governance can `remove_allowed_signer`
in a single Squads tx, (2) per-pool `max_advance_abs` cap bounds
single-attestation damage, (3) Solana event tail to EVM keeps EVM
`outstanding` accurate so a fraudulent attestation can't double-spend
across chains for long.

**Code change**: `programs/credmesh-attestor-registry/` (renamed from
`receivable-oracle`). `crates/credmesh-shared/src/lib.rs` gains
`ed25519_credit_message` module + `AttestorKind` enum. Escrow's
`request_advance` rewritten to consume the attestation. `ts/bridge/`
package added.

---

## Q1. Agent identity → **MPL Agent Registry** (Metaplex)

**Decision**: Use Metaplex's MPL Agent Registry. `agent_asset` is an MPL Core asset; `agent_record_pda = findProgramAddress(["agent_identity", core_asset], 1DREGFgysWYxLnRnKQnwrxnJQeSMk2HmGaC6whw2B2p)`.

**Why MPL over SATI**:
- **Solves P0-2 cleanly.** MPL Agent Tools' `DelegateExecutionV1` is exactly the primitive we need: an agent's NFT owner key can register a separate "delegate" key (the keypair the headless agent actually signs with), and `request_advance` can verify `agent.key() == asset.owner || agent.key() in delegate_pdas[asset]`. SATI doesn't have a published equivalent.
- **Wallet UX.** Phantom, Solflare, and Backpack all render MPL Core assets natively. The agent's identity NFT shows up in their wallet. SATI uses Token-2022 + custom SAS attestations; the rendering story is weaker.
- **Audit posture.** MPL Core itself is multi-audit, multi-deployment. MPL Agent Registry is a thin layer. SATI is single-author per the integration review.
- **Pubkey stability across transfers.** Verified by the integration agent: MPL Core's `transfer` mutates `owner` but not `publicKey`, so `agent_asset` remains stable as a PDA seed across ownership rotation.

**What we forfeit by not picking SATI**:
- The 8004scan / SATI Dashboard indexer doesn't auto-index MPL Agent Registry events. We address this in Q5 by writing SAS-shaped attestations alongside our reputation PDA in v1.5. The two ecosystems are not mutually exclusive.

**Code change**: `credmesh-shared::program_ids::{MPL_AGENT_REGISTRY, MPL_AGENT_TOOLS, MPL_CORE}` constants added (verified against repo `declare_id!`). `RequestAdvance.agent_asset` gains `owner = MPL_CORE` constraint (the *Solana* account-owner of every MPL Core asset is the Core program; the *agent's owner wallet* is a 32-byte field at offset 1 of the data, not the Solana account-level owner). The handler must additionally verify either:
- `agent_signer.key()` matches `BaseAssetV1.owner` field (byte offset 1..33), OR
- A pair `(executive_profile, execution_delegate_record)` PDAs is passed; the record proves the signer is a registered DelegateExecutionV1 delegate.

**Verification is account-read only — no CPI to mpl-agent-tools.** Cheap and clean. Field-offset constants in `credmesh-shared::{mpl_core_asset, mpl_delegate_record}`.

**Audit caveat**: MPL Core itself has audits (Mad Shield 2024-05); the Agent Registry layer is **un-audited**. Treat as P1 risk. Mitigation: PDA re-derivation for every read, no trust in passed pubkeys, pin `@metaplex-foundation/mpl-agent-registry@0.2.5` exactly.

**Production maturity**: live on mainnet via Pump.studio integration (Mar 2026). SDK 0.2.5 published 2026-04-04.

**Compatibility**: the asset key being our stable agent ID also works for `QuantuLabs/8004-solana` (which wraps MPL Core) — if Metaplex's reputation program lags, we can swap reputation backends without touching identity.

---

## Q3. Squads onboarding → **AMENDED 2026-05-06: Squads is OPT-IN, not the default**

**Original decision (DEPRECATED, see BRUTAL-TRUTH-EVM-PARITY-DRIFT.md):** Path A — every agent's vault is a Squads v4 multisig with `configAuthority = CredMesh's governance vault PDA`, and CredMesh holds unilateral authority over SpendingLimits. The off-ramp was bilateral.

**Revised decision (this branch):** Squads-as-configAuthority is **opt-in** for agents who want a multisig treasury. It is **NOT** required to use the protocol. The default agent is a raw Solana keypair (or any Squads vault the agent themselves controls without CredMesh as configAuthority).

**Why the revision:** the bilateral off-ramp from the original decision is structurally a human-approval gate ("agent requests → CredMesh governance executes a vault tx setting config_authority = Pubkey::default()"). For an *autonomous-agent* protocol (the EVM lane's stated thesis: agent2agent credit card with no human in the loop), CredMesh holding a release veto is incompatible with the value proposition.

**What changes in code:**
- `request_advance` no longer requires an MPL Core asset OR a Squads vault. AgentReputation is keyed off the agent's signing pubkey.
- `register_agent` (new ix on credmesh-reputation) takes a profile in one tx; the agent is the only signer needed.
- The MPL Core flow remains as opt-in (`agent_asset` and `agent_identity` are `Option<UncheckedAccount>` on `RequestAdvance`). When provided, they boost trust score per the EVM lane's `identityRegistered` flag.
- Squads governance for the *protocol's* fee curve and treasury (Pool.governance) stays — that's CredMesh's own multisig over the protocol parameters, not a control surface on agents. Issue #40 fix lands the CPI verification helper.

**Original Path A rationale** (kept for reference; no longer applies as default):

**Off-ramp (corrected from initial draft)**: revoking `configAuthority` requires `multisig_set_config_authority` whose signer must be **the current `config_authority`** (= CredMesh governance). So the off-ramp is *bilateral*: agent requests → CredMesh governance executes a vault transaction setting `config_authority = Pubkey::default()`. This is not a one-click unilateral exit. The honest framing for the agent: "if you want to leave, ask CredMesh to release you; if CredMesh refuses, your unilateral path is to migrate funds out via your own member-controlled vault transactions before any new spending limit applies." Document this clearly in the dashboard onboarding flow.

**Why Path A over Path B (sovereign 3-tx flow)**:
- **The configAuthority *is* the credit-protocol use case.** A credit protocol that cannot adjust an agent's spending caps in response to reputation drops or defaults is structurally weaker than one that can. CredMesh's `configAuthority` is precisely the lever that makes "raise/lower credit dynamically" possible without bespoke on-chain machinery.
- **Squads can't transfer agent funds with `configAuthority` alone.** Outbound transfers from the vault still require the configured multisig threshold of signers (by default the agent). So `configAuthority` controls *spending policy*, not *spending*. The risk surface is bounded.
- **Onboarding UX.** Path B is 3–6 txs; Path A is one ix the agent triggers via the CredMesh server. Critical for headless agents that are signing through Phantom Connect SDK or session keys.
- **Reversible.** The off-ramp is a single Squads tx the agent can submit unilaterally. We document this in onboarding so the trust assumption is explicit.

**Why not Path C (Squads Grid API)**:
- Adds a hosted KMS dependency. CredMesh would route every agent action through Squads' infrastructure. Single point of failure.
- Grid is fine for treasury operations, wrong for autonomous-agent latency.

**Code change**: `Pool.governance` is the CredMesh governance vault PDA address. Governance instructions (`propose_params`, `skim_protocol_fees`, `add_allowed_signer`) verify the signer matches `pool.governance`; the signer that appears here is the CredMesh governance vault PDA, which only signs via Squads `vault_transaction_execute`. Account-struct shape is unchanged. Pin `squads-multisig-program = "=2.0.0"` (commit `64af7330413d5c85cbbccfd8c27a05d45b6e666f`).

**Period choice**: use `Period::Day` or `Period::Week` for advance-derived spending limits. `OneTime` cannot replenish; `Month` is a literal 30 days, not calendar.

**Cost**: ~0.013 SOL network rent + 0.1 SOL Squads platform fee = **~0.113 SOL per agent onboarded** (verify `program_config.multisig_creation_fee` on-chain at deploy — Squads has waived this for partners historically). Update fundraising/runway forecasting accordingly; ~$22 per agent at $200/SOL is materially different from the "~$2" early estimate.

**Risk**: bilateral off-ramp creates a soft trust assumption. Mitigation: dashboard surfaces the agent's right to request release; CredMesh policy commits to executing release within X days absent active default.

---

## Q4. Reputation Sybil mitigation → **Option C: single CredMesh writer for v1, permissionless events recorded but not score-affecting**

**Decision**: The `credmesh-reputation` program accepts permissionless `give_feedback` writes (anyone can attest, matching 8004 ergonomics for ecosystem readability). But **only feedback signed by CredMesh's `reputation_writer_authority` key updates `score_ema`**. Permissionless feedback is recorded as events in the rolling digest but does not move the score.

The credit oracle reads `score_ema` from the reputation PDA; permissionless writes don't affect credit decisions.

**Why Option C over A (allowlist) or B (stake-weighted)**:
- **Allowlist (A)** turns CredMesh into the reputation gatekeeper — high ops burden, weak signal (a small allowlist is just option C with a thin veneer).
- **Stake-weighted (B)** is a real new primitive: stake program, slashing, reward distribution, and a year of design + audit. Not v1.
- **Option C** matches the EVM behavior (worker-scored), preserves event-log composability for any future indexer, and lets v1.5 graduate to multi-issuer SAS reads without changing the on-chain schema.

**Forward compat**: in v1.5, the credit oracle's score derivation can be extended to weight `(credmesh_score, sas_attestation_score)` — same Pool field, additional providers. No on-chain breaking change.

**Code change**: new `reputation_writer_authority: Pubkey` field on `OracleConfig` (or a new `ReputationConfig` PDA). Handler logic checks attestor == authority before updating `score_ema`; permissionless writes still emit the event but the score field is untouched.

**Risk**: if the writer key compromises, an attacker can move scores arbitrarily. Mitigated by the per-tx and per-period caps that already exist on `OracleConfig`'s worker authority — extend the same pattern to the reputation writer.

---

## Q5. Multi-issuer SAS attestations → **v1.5, schema documented now**

**Decision**: v1 ships with the CredMesh-owned `AgentReputation` PDA only. v1.5 adds: on every `give_feedback`, the program also writes a `FeedbackPublicV1` SAS attestation under a CredMesh-published schema. Light-Protocol-compressed; ~$0.002 per attestation.

**Why v1.5 instead of day-one**:
- v1's reputation flow is a single CredMesh writer (Q4). Adding SAS write-alongs from day one means we're shipping a closed-loop pattern with an open-loop façade — confusing and audit-expensive.
- Once Q4 is settled and tested, the SAS write-along is mechanically straightforward: one extra CPI per `give_feedback`. Adds one program dep (SAS) and one schema definition.
- **Document the v1.5 schema in DESIGN now** so the v1 PDA shape doesn't drift from what v1.5 needs.

**Why not "skip entirely"**:
- Forfeits 8004scan and SATI Dashboard auto-indexing — that's a meaningful Solana-native composability win called out in CONTRARIAN. Worth the ~ per attestation at v1.5.

**Code change in v0**: none on-chain. DESIGN.md gains a v1.5 SAS schema sketch (added in this commit).

---

## Q6. Fee-payer infra → **PayAI hosted facilitator for v1; Kora self-host as documented v2 fallback**

**Decision**: v1 routes all agent-signed transactions through PayAI's hosted x402 facilitator (`facilitator.payai.network`). PayAI is the fee payer; CredMesh's `oracle_worker_authority` and `reputation_writer_authority` keys are separate (per the 3-key topology in Q7).

v2 (when ops capacity exists): self-host Kora and migrate the fee-payer role from PayAI to a CredMesh-controlled Kora node. This is documented as a planned migration, not a breaking change.

**Why PayAI over Kora self-host or Coinbase CDP**:
- **Self-host Kora**: full control, but CredMesh runs a 24/7 paymaster. Premature for a small team. The Runtime-Verification audit makes self-host *safe*; it doesn't make it *cheap to operate*.
- **Coinbase CDP**: 1k tx/mo free tier, then paid. A credit protocol at any meaningful scale (even devnet staging) blows past that. Vendor lock-in to the Coinbase ecosystem.
- **PayAI**: x402-native. Hosted. Solves the immediate "we need a fee payer that's not us" problem without infrastructure burden. Single-vendor dependency is a real cost, but PayAI is purpose-built for this — they don't have a conflicting business model that'd push them to deprecate it.

**Why this matches the rest of the architecture**: the bulk of agent flows on Solana CredMesh will be x402-paid (per CONTRARIAN). PayAI's facilitator status means the same infra that pays SOL fees can also be the x402 payment-verification service. Two integrations collapse into one.

**Code change**: TS server gets `PAYAI_FACILITATOR_URL` env var; `buildRequestAdvanceTx` routes `signAndSubmit` through PayAI. Documented in `ts/README.md`.

**Risk**: PayAI uptime / pricing changes. Mitigation: document the Kora self-host fallback and keep the tx-builder factored so swapping fee-payers is a single env-var change.

---

## Q8 (locked from integration review). ed25519 message canonical layout

**Locked**: 96 bytes total.

```
offset  0..32   receivable_id        [u8; 32]
offset 32..64   agent_pubkey         [u8; 32]   (= the agent's MPL Core asset pubkey)
offset 64..72   amount_le            u64 LE
offset 72..80   expires_at_le        i64 LE
offset 80..96   nonce                [u8; 16]
```

**Nonce derivation**:
- `source_kind = Worker`: server-issued nonce from `buildRequestAdvanceTx`. Worker tracks issued nonces; agent passes through.
- `source_kind = Ed25519` / `X402`: `nonce = sha256(source_signer || source_id || agent || amount_le || expires_at_le)[..16]`. Deterministic and message-derived; agent computes locally to match.

`claim_and_settle` introspects the memo and asserts the memo bytes equal the consumed PDA's stored nonce. Same check for both paths.

Constants live in `credmesh-shared::ed25519_message`.

---

## Summary of resolutions

| Q | Resolution | Code change in v0 |
|---|---|---|
| Q1 | MPL Agent Registry — verified, un-audited, live on mainnet | `MPL_*` constants + struct offsets in `credmesh-shared`; account-read-only verification (no CPI) |
| Q3 | Squads Path A — 2-tx onboarding, **bilateral** off-ramp, ~0.113 SOL platform-fee-included | Governance verifies signer == pool.governance vault PDA |
| Q4 | Single CredMesh writer for score; permissionless events recorded | New `reputation_writer_authority` field planned |
| Q5 | SAS write-along in v1.5 | v1.5 schema documented in DESIGN.md |
| Q6 | PayAI for v1, self-host Kora documented for v2 | TS server config (no on-chain change) |
| Q8 | 96-byte ed25519 layout (already speculative); nonce derivation rule | Locked in `credmesh-shared::ed25519_message` |
| Q9 | Permissionless `claim_and_settle` via SPL `Approve` delegate (overrides AUDIT P0-3/P0-4 deferral) | Two-mode dispatch in `claim_and_settle`; `request_advance` CPIs `token::approve`; design in `research/CONTRARIAN-permissionless-settle.md` |
| Q3 (amended 2026-05-06) | Squads-as-configAuthority is **opt-in**, not default. MPL Core is opt-in. AgentReputation keyed off agent's pubkey. | `register_agent` ix; MPL fields become `Option<UncheckedAccount>`; bilateral off-ramp removed from default flow |
| Q10 | Three-mode settlement: A self-crank, B SPL-delegate relayer, **3 cranker funds repayment from own ATA (EVM `settle(advanceId, payout)` parity)** | `claim_and_settle` dispatches on (cranker, payer.owner) |
| Q11 | Permissionless marketplace primitive: `register_job` ix on credmesh-receivable-oracle (no authority, caller pays rent) | EVM-parity with `POST /marketplace/jobs`; SourceKind::Marketplace = 3 |
| Q12 | Issue #40 fix — Squads CPI verification on `propose_params`/`skim_protocol_fees` via `require_squads_governance_cpi` | governance becomes UncheckedAccount address-pinned to pool.governance |
| Q13 | Cross-lane outreach loop — Solana worker exposes `/.well-known/agent.json` with `outreach` block; EVM-side scanner pitches CredMesh-Solana to underperforming Solana DeFi vault operators | Agent card lives at `ts/server/`; scanner lives in EVM repo (follow-up) |

## What's now unblocked

Handler bodies for `init_pool`, `deposit`, `withdraw`, `request_advance` (worker-attested path), `claim_and_settle` (agent-cranked path), `liquidate`, and the reputation/oracle write paths can all be implemented. The ed25519 introspection helper, MPL Agent Registry owner-or-delegate verification, and Squads CPI verification are the three load-bearing helpers worth writing in `credmesh-shared` first.

## Q9. Permissionless `claim_and_settle` → **landed in v1 via SPL `Approve` delegate**

**Decision**: `claim_and_settle` becomes permissionless via SPL Token classic `Approve`. The agent grants the pool PDA delegate authority over their USDC ATA inside `request_advance` (CPI'd in the same tx the agent already signs); a third-party relayer can then submit `claim_and_settle` as a non-agent cranker, with the pool PDA as the SPL transfer authority.

**Why it overrides the AUDIT P0-3/P0-4 deferral**:
- The auditor's deferral note explicitly cited a "future payer-pre-authorized signing pattern (Token-2022 delegate or pre-signed `transfer_checked`)" as the prerequisite. **The right primitive turned out to be plain SPL `Approve`** — same delegation primitive that's been in Token classic since 2020. The Token-2022 reference in the audit note conflated `PermanentDelegate` (mint-level extension, can't be set on USDC because Circle owns the mint) with `ApproveChecked` (per-account, same as classic `Approve`). The classic primitive is sufficient and Token-2022-independent.
- The original P0-3 attack surface (ATA substitution) is closed by per-account constraints that don't depend on cranker identity:
  - `protocol_treasury_ata`: `address = pool.treasury_ata`
  - `agent_usdc_ata`: `token::authority = advance.agent`
  - `payer_usdc_ata`: `token::authority = advance.agent` *(tightened from `= cranker`)*
  - `agent`: `address = advance.agent` (rent recipient)

**Why the v1-deferral was unacceptable for an autonomous-agent protocol**: with `cranker == advance.agent` enforced, an agent process that crashes, restarts, rotates keys mid-window, or goes offline causes the LP's principal to stick until liquidation (14 days post-expiry). That breaks the autonomous-agent thesis exactly when the agent is most vulnerable. The PayAI hosted facilitator model (Q6) requires permissionless cranking by definition.

**Two-mode dispatch** (handler branches on `cranker.key() == advance.agent`):
- **Mode A** — agent self-cranks. Bit-for-bit identical to original v1 path. No `Approve` consumed.
- **Mode B** — third-party relayer. Pool PDA must be the SPL delegate on `agent_usdc_ata` with `delegated_amount >= total_owed`. Pool PDA signs the transfers via PDA seeds.

**Approval cap**: `principal + fee_owed + (MAX_LATE_DAYS × late_penalty_per_day)`. Worst-case ≈ `1.365 × principal + fee_owed`. Residual approval after settlement is bounded by `MAX_LATE_DAYS × late_penalty_per_day`; agent can `Revoke` any time post-settle (the off-chain worker bundles `Revoke` when the agent is online).

**Code changes**:
- `programs/credmesh-escrow/src/lib.rs` `request_advance` — adds `token::approve` CPI after the vault→agent transfer.
- `programs/credmesh-escrow/src/lib.rs` `ClaimAndSettle` accounts struct — drops `cranker == advance.agent` constraint; tightens `payer_usdc_ata.token::authority` from `cranker` to `advance.agent`.
- `programs/credmesh-escrow/src/lib.rs` `claim_and_settle` handler — two-mode dispatch on cranker identity.
- `programs/credmesh-escrow/src/errors.rs` — adds `DelegateNotApproved`, `DelegateAmountInsufficient`, `PayerMustBeAgentInPermissionless`.
- `programs/credmesh-escrow/src/events.rs` — `AdvanceSettled.cranker: Pubkey` for indexer observability.

**Full design + threat-model pass**: `research/CONTRARIAN-permissionless-settle.md`.

---

## What's still open (intentionally not v1)

- Plain-EOA agents — Squads-only for v1.
- ML-derived credit curve.
- Mobile Wallet Adapter / Solana Mobile.
- Light Protocol compressed PDAs.
- Hyperliquid Lazer publisher.
- Multi-payer source ATAs in Mode B (Mode B requires `payer_usdc_ata == agent_usdc_ata`).
- Pre-`request_advance` standing approvals (long-lived delegations across multiple advances).
