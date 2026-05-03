# AUDIT — Consolidated review of DESIGN.md + scaffold

Synthesizes three independent reviews of the v0 scaffold:
- **Security audit** (Anchor footguns) — 6 P0, 6 P1, 12 P2 findings
- **Account model & tx-packing** — 10 fixes, mostly seed design and constraints
- **Integration coherence** — verified each external-protocol claim individually; 8 "killer questions" answered

This document is the **single source of truth** for what changes before handler bodies are written. Where it conflicts with DESIGN.md or the research, AUDIT.md wins.

---

## Critical (fund-loss) — P0

### P0-1. `Liquidate` missing `consumed.agent == advance.agent` constraint
File: `programs/credmesh-escrow/src/lib.rs` (`Liquidate` struct)
**Status: FIXED in this commit.** One-line constraint added.

### P0-2. Agent identity binding is broken
`request_advance` has `agent: Signer` and `agent_asset: UncheckedAccount` with **no constraint tying them**. Any keypair can claim to own any agent asset. Combined with the reputation read pattern, an attacker pre-seeds a high-score Reputation PDA for an attacker-controlled `agent_asset`, signs with any keypair, and gets max credit.

**Decision required from team — see "Open design questions" below.** Cannot fix mechanically until MPL Agent Registry vs SATI is chosen.

**Status: BLOCKED on decision.** Marked with `// AUDIT P0-2:` comment in source.

### P0-3. `claim_and_settle` destination ATAs unconstrained
`agent_usdc_ata`, `protocol_treasury_ata`, `payer_usdc_ata` are bare `mut` `TokenAccount`s. A cranker can substitute attacker-owned ATAs and steal:
- The 15% protocol cut (substitute `protocol_treasury_ata`)
- The agent net (substitute `agent_usdc_ata`)
- Drain a victim's USDC (substitute `payer_usdc_ata` if signing logic is loose)

**Status: PARTIALLY FIXED in this commit.**
- Added `treasury_ata: Pubkey` field to `Pool`.
- `protocol_treasury_ata` now `address = pool.treasury_ata`.
- `agent_usdc_ata` now `token::mint = pool.asset_mint, token::authority = advance.agent` (pending P0-2 decision on what `advance.agent` actually is).
- `payer_usdc_ata` requires `cranker.key() == advance.agent` for now (agent-self-crank only). Permissionless cranker support deferred to v2 with explicit payer-binding on the receivable.

### P0-4. `payer_usdc_ata` no signer-authority binding
Same fix as P0-3: in v1, only `advance.agent` can be the cranker. Permissionless settle requires a future "payer-pre-authorized" pattern (Token-2022 delegate or pre-signed `transfer_checked`). **Status: FIXED via P0-3 fix.**

### P0-5. `ConsumedPayment` close-then-reinit replay
Closing `ConsumedPayment` at settle/liquidate destroys replay protection. Attacker bundles `[liquidate(X), request_advance(receivable_id=X)]` in one tx; the closed PDA re-inits with the same `receivable_id`.

**Status: FIXED in this commit.** `ConsumedPayment` is now permanent — never closed. Cost: ~0.0017 SOL rent stuck per receivable. Acceptable; the agent already pays this and it's the only safe option per the security review.

### P0-6. `paused` field with no enforcement
Dead surface that violates the design invariant ("issuance is never paused") if anyone wires it up later.

**Status: FIXED in this commit.** Removed `paused` field from `Pool`, removed `PauseScopeViolation` error.

---

## High-severity — P1

### P1-1. Cross-program `UncheckedAccount` reads need a deserialize plan
Escrow reads `AgentReputation` (owned by `credmesh-reputation`) and `Receivable` (owned by `credmesh-receivable-oracle`) via `UncheckedAccount`. Handler must (1) verify owner pubkey, (2) re-derive PDA, (3) check 8-byte discriminator, (4) deserialize. Forgetting any is a Wormhole-class bug.

**Status: PARTIAL — added `credmesh-shared` crate to centralize seed constants and program IDs. Deserialize helper to be added in handler-implementation phase. Documented in `programs/credmesh-shared/src/lib.rs`.**

### P1-2. `instructions_sysvar` not address-constrained — sysvar spoofing
**Status: FIXED in this commit.** All three sites (`RequestAdvance`, `ClaimAndSettle`, `Ed25519RecordReceivable`) now have `address = solana_program::sysvar::instructions::ID`.

### P1-3. `init_if_needed` feature flag missing in Cargo.toml
Anchor 0.30 puts `init_if_needed` behind a feature flag. Code uses it; build will fail without it.
**Status: FIXED in this commit.**

### P1-4. `AllowedSigner` self-referential seed
Seed is `allowed_signer.signer.as_ref()` — read from the very account being seed-validated. Need to pass `signer_pubkey` as ix arg.
**Status: FIXED in this commit.** Instruction signature updated; seed now uses ix-arg pubkey.

### P1-5. Reputation Sybil — `give_feedback` writes are fully permissionless with no per-attestor tracking
The current `AgentReputation` struct stores a rolling `feedback_digest` but doesn't record per-feedback attestor. An attacker self-attests `score=100` from 1000 keys to inflate `score_ema`. Escrow has no allowlist filter.

**Decision required — see "Open design questions" below.**

**Status: BLOCKED on decision.** Marked with `// AUDIT P1-5:` comment in source.

### P1-6. `init_pool` "governance" is just any signer
A Squads vault is a PDA — it cannot be a `Signer`. The current scaffold lets whoever runs `init_pool` first become governance.

**Decision required — see "Open design questions" below.**

**Status: BLOCKED on decision.** Marked with `// AUDIT P1-6:` comment in source.

---

## Account model fixes (mostly mechanical)

### AM-1. Add `pool` to `Advance` and `ConsumedPayment` seeds
Future-proofs multi-pool collisions; cheap now, expensive to retrofit.
**Status: FIXED in this commit.**
- `Advance`: `[ADVANCE_SEED, pool.key().as_ref(), agent.key().as_ref(), receivable_id]`
- `ConsumedPayment`: `[CONSUMED_SEED, pool.key().as_ref(), agent.key().as_ref(), receivable_id]` — issue #8 (post-fix). The `agent` seed eliminates cross-agent `receivable_id` DoS-collisions; previously two agents picking the same 32-byte `receivable_id` collided on the same PDA and the second `request_advance` failed. Test fixture: `tests/bankrun/attacks/cross_agent_receivable_id_reuse.test.ts`.

### AM-2. Constrain `agent_asset` ownership
**Status: BLOCKED on P0-2 decision.** Marker comment in source.

### AM-3. Set explicit CU limit (180k) in TS tx-builder
**Status: NOTED for tx-builder phase. Documented in `ts/README.md`.**

### AM-4. Establish per-Pool ALT at deploy time
**Status: NOTED for deploy script phase.**

### AM-5. Lazy period reset in `ed25519_record_receivable`
4 lines at top of handler. **Status: NOTED for handler-implementation phase.** Documented in source comment.

### AM-6. Extract `credmesh-shared` seeds crate
**Status: FIXED in this commit.** New `programs/credmesh-shared/` crate with `seeds.rs` and `program_ids.rs`.

### AM-7. Don't close `Advance` on `Liquidate` — keep audit trail
**Status: FIXED in this commit.** `Liquidate` no longer closes `Advance`; updates `state = Liquidated` only. `ConsumedPayment` is permanent (per P0-5).

### AM-8. Document early-liquidation lever in DESIGN §3.5
**Status: FIXED in this commit.** Added to DESIGN.md.

### AM-9. v2 tier-pool seed shape
**Status: FIXED in this commit.** Added paragraph to DESIGN.md §9.

---

## Open design questions — RESOLVED in [DECISIONS.md](./DECISIONS.md)

The five blocking questions (Q1, Q3, Q4, Q5, Q6) plus the ed25519 layout (Q8) have been answered. Summary:

- **Q1**: MPL Agent Registry (Metaplex), not SATI. `DelegateExecutionV1` solves the agent-binding hole cleanly.
- **Q3**: Squads Path A (Controlled Multisig + sovereignty off-ramp). The configAuthority IS the credit-protocol use case.
- **Q4**: Single CredMesh writer updates `score_ema`; permissionless `give_feedback` writes are recorded but score-inert.
- **Q5**: SAS multi-issuer attestations land in v1.5; schema documented now to prevent breaking change.
- **Q6**: PayAI hosted facilitator for v1; self-host Kora as documented v2 fallback.
- **Q8**: 96-byte ed25519 message layout locked. Constants in `credmesh-shared::ed25519_message`.

See [DECISIONS.md](./DECISIONS.md) for rationale on each call. Original framing of the questions retained below for traceability.

### Q1. MPL Agent Registry vs SATI for agent identity (P0-2, AM-2)
Two competing 8004-on-Solana implementations exist:
- **MPL Agent Registry**: Metaplex-published, programs `1DREGFgysWYxLnRnKQnwrxnJQeSMk2HmGaC6whw2B2p` (Identity) + `TLREGni9ZEyGC3vnPZtqUh95xQ8oPqJSvNjvB7FGK8S` (Tools). Uses MPL Core asset; `agent_record_pda = findProgramAddress(["agent_identity", core_asset], 1DREG…)`. Has a `DelegateExecutionV1` flow for binding non-owner keys.
- **Cascade SATI**: Token-2022 + SAS-based, `satiRkxEiwZ51cv8PRu8UMzuaqeaNU9jABo6oAFMsLe`. Aligns with the SAS attestation ecosystem; has a published indexer (`8004scan`).

**Decision needed**: which program is the authoritative `agent_asset` source? Pick one and update `DESIGN.md` §2 + the `RequestAdvance` constraint.

### Q2. ConsumedPayment policy — confirmed permanent (no decision needed; documented for record)
Closed forever per P0-5 fix. Rent stays locked. Trade-off accepted.

### Q3. Squads-as-governance integration pattern (P1-6)
Squads onboarding is **not single-tx** (per integration review killer #1):
- Path A: "Controlled Multisig" with `configAuthority = CredMesh` — single ix to add SpendingLimit, but CredMesh has unilateral config power over the agent's vault. Contradicts agent sovereignty.
- Path B: Full multisig flow — `multisigCreateV2` → `vaultCreate` → `ConfigTransaction` → `Proposal` → `Approve` → `Execute`. ~6 txs end-to-end, agent retains sovereignty.
- Path C: Squads Grid API (hosted, KMS-backed) — abstract the ix flow but adds a hosted dependency.

**Decision needed**: which onboarding path? Affects `init_pool` governance wiring and the agent-onboarding UX in `ts/server`.

### Q4. Reputation Sybil mitigation (P1-5)
- **Option A**: Add per-attestor allowlist on `Pool`. Escrow filters `AgentReputation.score_ema` only when computed from allowlisted attestors. Operationally heavy; CredMesh becomes the allowlist gatekeeper.
- **Option B**: Stake-weighted reads. Attestors must lock SOL/USDC; their feedback weight scales with stake. Adds a third on-chain primitive.
- **Option C**: Single-writer (CredMesh worker) for v1; multi-issuer SAS deferred to v1.5. Loses interop but ships fast.

**Decision needed**: A, B, or C?

### Q5. Multi-issuer SAS reputation roadmap (Integration killer #7)
CONTRARIAN.md advocated for SAS attestations alongside the CredMesh-owned PDA — costs ~$0.002/attestation via Light compression but gives 8004scan / SATI Dashboard automatic indexing.

**Decision needed**: write SAS attestations from day one (interop), defer to v1.5 (faster ship), or skip entirely (closed-loop)?

### Q6. Kora self-host vs PayAI/Coinbase CDP (Integration #6)
Kora is a self-hosted SDK, not a hosted service. The "Kora facilitator" wording in DESIGN was wrong (per REVIEW.md material error #2 + this audit).
- **Self-host Kora**: Runtime-Verification audited, full control, you own ops.
- **PayAI**: hosted at `facilitator.payai.network`, x402-native.
- **Coinbase CDP**: free 1k tx/mo, then paid.

**Decision needed**: pick one for v1. Affects key topology (see Q7).

### Q7. Worker key topology — three keys, not one (Integration killer #6)
Required separation:
1. **Fee-payer key** (Kora signer) — low value, hot, rotating.
2. **Oracle worker authority** — writes `Receivable` PDAs for `source_kind=0`. High value if compromised; capped per-tx and per-period via `OracleConfig`.
3. **Reputation provider key** — signs `ReputationScoreV3` records (only if Q5 = "yes/v1.5").

**Status: NEEDS DOCUMENTING.** Will add §3.8 threat-model section to DESIGN.md once Q6 is decided.

### Q8. ed25519 message canonical layout (Integration #4)
Recommended: 96 bytes = `receivable_id (32) || agent_pubkey (32) || amount_le (8) || expires_at_le (8) || nonce (16)`. Nonce derives from `sha256(source_signer || source_id || agent || amount || expires_at)[..16]` for x402 path; server-issued for worker path. Must be locked before handler bodies.

**Status: PROPOSED LAYOUT in DESIGN §3.4 update; awaiting team confirmation.**

---

## Things that look right (calibration)

- Account-size math on all 8 accounts is consistent and includes 8-byte Anchor discriminator + reasonable padding (verified field-by-field).
- No struct-name discriminator collisions across the three crates.
- `close = agent` (not cranker) on `Liquidate` and `claim_and_settle` correctly neutralizes MEV-driven cranking.
- `init` semantics for `ConsumedPayment` correctly enforce replay-by-uniqueness.
- `address = pool.usdc_vault`, `pool.share_mint`, `pool.asset_mint` constraints prevent mint/vault swaps.
- `overflow-checks = true` in release profile (most teams forget this).
- Two-step timelock via `propose_params` / `execute_params` is the standard Squads-friendly pattern.
- Virtual-shares offset (1e6 / 1e9) is the right Solana-equivalent of ERC-4626 `_decimalsOffset=6` first-depositor defense.
- `PROTOCOL_FEE_BPS = 1500` matches the EVM 85/15 split.
- Tx-packing: `request_advance` is ~14-15 accounts — fits in v0 without an ALT.
- v1 throughput ceiling (single-Pool write-lock): ~30-80 advances/sec, well above realistic v1 demand.

---

## Pre-coding tasks (sequenced)

Before any handler body is written:

1. **Resolve Q1** (MPL vs SATI). Update DESIGN §2 + add `agent_asset.owner` constraint to `RequestAdvance`. Spike both SDKs in 1 day if unsure.
2. **Resolve Q3** (Squads onboarding path). Spike a devnet flow that creates a Squads multisig + vault + spending-limit for one fake agent and measures cost.
3. **Resolve Q4** (Reputation Sybil) — pick A, B, or C.
4. **Resolve Q5** (SAS write-along) — yes/v1.5/no.
5. **Resolve Q6** (Kora self-host vs PayAI vs CDP) — pick one.
6. **Confirm Q8** (ed25519 message layout) — lock the 96-byte format.
7. **Add §3.8 threat-model** to DESIGN.md once Q6 is settled (key topology).
8. **Build a Bankrun cross-agent-replay test fixture** that confirms the agent-identity binding from Q1 actually closes the hole.
9. **Pin Squads v4 + MPL Agent Registry verified-build commits** in `Anchor.toml`-adjacent README.

Once 1–9 are answered, handler bodies can be written safely.

---

## Post-EPIC #9 audit pass (2026-04-29 → 2026-05-03)

After the EPIC #9 multi-track engineering work landed, a 5-pass audit was run on the merged main:

| Lens | Auditor | Verdict |
|---|---|---|
| AUDIT-invariants on full merged tree (P0/P1/P2 + AM-1..7) | Claude code-reviewer | SECURE |
| Reputation handler correctness (post-#11/#12/#28/#30) | Claude code-reviewer | PASS |
| Cross-program correctness self-audit on PR #30 | Claude code-reviewer | SHIP |
| Test coverage gap analysis vs AUDIT findings | Claude code-reviewer | scaffolds shipped; behavioral activation pending IDL fix #15 |
| Adversarial bug-hunt (independent model) | Kimi K2 via forge | 2 HIGH + 5 MED + 2 LOW raised |

### Findings from the independent-model pass

**Verified false positives (after source verification):**
- *HIGH #1*: `skim_protocol_fees doesn't decrement pool.total_assets`. Misread of dual-ledger design — `total_assets` and `accrued_protocol_fees` are intentionally separate (claim_and_settle line 522-528). The actual invariant is `deployed_amount ≤ total_assets`, not `vault + deployed >= total_assets`. Skim correctly only decrements `accrued_protocol_fees`.
- *HIGH #2*: `liquidate cascading from #1`. Cascade from a non-bug; `total_assets >= deployed_amount` is invariant-enforced, and any outstanding principal is part of `deployed_amount`.

**Verified real findings — all FIXED in PR #32:**
- *MED #3*: `worker_update_receivable` and `ed25519_record_receivable` both used `init_if_needed` on identical seeds `[RECEIVABLE_SEED, agent, source_id]`, allowing one path to overwrite the other's data. **Status: FIXED.** Receivable PDAs now namespaced by `source_kind` (`[RECEIVABLE_SEED, source_kind_byte, agent, source_id]`). Worker hardcodes `&[0u8]`; ed25519 uses `&[allowed_signer.kind]` (1 = exchange, 2 = x402) — `kind` sourced from the on-chain `AllowedSigner`, not the ix arg.
- *MED #4*: `require_memo_nonce` looped over all tx instructions unboundedly — DoS vector once `claim_and_settle` becomes permissionless in v1.5. **Status: FIXED.** Capped at `MAX_IX_SCAN = 64` (above Solana's practical tx-size ix-count limit).
- *MED #5*: `init_pool` and `propose_params` accepted any `FeeCurve` without validating internal invariants. Governance footgun. **Status: FIXED.** New `FeeCurve::validate()` helper enforces `utilization_kink_bps ≤ BPS_DENOMINATOR`, `base_rate_bps ≤ kink_rate_bps ≤ max_rate_bps ≤ BPS_DENOMINATOR`. New `CredmeshError::InvalidFeeCurve` variant (appended; existing discriminants preserved). Called from both `init_pool` and `propose_params`.

**Compile-discovered finding — FIXED in PR #34:**
- `#[event_cpi]` and `emit_cpi!` (introduced by PR #11) are gated behind `anchor-lang`'s `event-cpi` Cargo feature. None of the 5 audit passes caught this because all reviewed source-only. The first `anchor build --no-idl` for the devnet deploy surfaced it. **Status: FIXED.** Workspace `Cargo.toml` adds `event-cpi` to `anchor-lang` features.

### Lessons from this audit cycle

1. **Source-only review has systematic blind spots.** Compile is the missing test. Future audits should include a `cargo check --workspace` or `anchor build` step.
2. **Independent-model audits catch what same-family audits miss.** Kimi K2's 3 real MED findings were missed by all 4 Claude reviewers. The cost: 1 HIGH false-positive that needed verification.
3. **Verification before patching is non-negotiable.** Both Kimi HIGHs would have led to wasted patch effort if accepted at face value. Source-trace each finding one to two hops before acting.
4. **Re-audit the patch.** PR #32's audit-driven fixes themselves got a dual re-audit (Claude code-reviewer + Kimi K2 on the diff). Both verdicts: SAFE TO MERGE, no regressions introduced.

The methodology pattern (parallel same-family audits + 1 independent-model audit + mandatory source verification) is documented as the `cross-model-code-audit` skill in `~/.claude/skills/`.
