# DECISIONS — Resolutions for AUDIT.md open design questions

The user delegated the call on Q1, Q3, Q4, Q5, Q6. Decisions below are made by the model from the context of the four research docs + REVIEW + CONTRARIAN + the three audit reviews. Reasoning is included so the team can override any individual call without losing the surrounding context.

These choices unblock handler-body implementation. Where the decision required code changes, the changes are in the same commit.

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

**Code change**: `credmesh-shared::program_ids::MPL_AGENT_REGISTRY` constant added; `RequestAdvance.agent_asset` will gain `owner = MPL_AGENT_REGISTRY` once the program ID is officially confirmed (placeholder used in v0).

**Risk**: if Metaplex meaningfully changes the Agent Registry shape, we eat a migration. Mitigation: pin a verified-build commit hash in our deploy doc.

---

## Q3. Squads onboarding → **Path A (Controlled Multisig with sovereignty off-ramp)**

**Decision**: Each agent's vault is a Squads v4 multisig with `configAuthority = CredMesh's governance multisig`. CredMesh holds unilateral authority to add/update SpendingLimit PDAs on the agent's vault. Onboarding is one CredMesh-authored ix after the agent creates the multisig.

The agent receives an explicit, documented off-ramp: a single Squads `configTransaction` (signed only by the agent) that sets `configAuthority = None`, severing CredMesh's authority. This is the agent's "exit" — taking it forfeits eligibility for further advances (CredMesh can no longer adjust their spending limits in response to defaults), but it makes the trust assumption explicit and reversible.

**Why Path A over Path B (sovereign 3-tx flow)**:
- **The configAuthority *is* the credit-protocol use case.** A credit protocol that cannot adjust an agent's spending caps in response to reputation drops or defaults is structurally weaker than one that can. CredMesh's `configAuthority` is precisely the lever that makes "raise/lower credit dynamically" possible without bespoke on-chain machinery.
- **Squads can't transfer agent funds with `configAuthority` alone.** Outbound transfers from the vault still require the configured multisig threshold of signers (by default the agent). So `configAuthority` controls *spending policy*, not *spending*. The risk surface is bounded.
- **Onboarding UX.** Path B is 3–6 txs; Path A is one ix the agent triggers via the CredMesh server. Critical for headless agents that are signing through Phantom Connect SDK or session keys.
- **Reversible.** The off-ramp is a single Squads tx the agent can submit unilaterally. We document this in onboarding so the trust assumption is explicit.

**Why not Path C (Squads Grid API)**:
- Adds a hosted KMS dependency. CredMesh would route every agent action through Squads' infrastructure. Single point of failure.
- Grid is fine for treasury operations, wrong for autonomous-agent latency.

**Code change**: `Pool.governance` is the CredMesh multisig pubkey. New `propose_params`-style instructions verify a Squads CPI by checking that the calling instruction's program ID is the Squads v4 program AND the signer is the CredMesh governance vault PDA. Implementation detail handled in handler bodies; account-struct shape is unchanged.

**Risk**: the off-ramp is documented but not on-chain-enforced. Mitigation: add a documented "agent revocation" flow to the dashboard so the agent can find the off-ramp without reading source.

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
| Q1 | MPL Agent Registry | `MPL_AGENT_REGISTRY` const added; `agent_asset` owner constraint planned |
| Q3 | Squads Path A (Controlled Multisig + off-ramp) | Governance handler will verify Squads CPI; account shape unchanged |
| Q4 | Single CredMesh writer for score; permissionless events recorded | New `reputation_writer_authority` field planned |
| Q5 | SAS write-along in v1.5 | v1.5 schema documented in DESIGN.md |
| Q6 | PayAI for v1, self-host Kora documented for v2 | TS server config (no on-chain change) |
| Q8 | 96-byte ed25519 layout (already speculative); nonce derivation rule | Locked in `credmesh-shared::ed25519_message` |

## What's now unblocked

Handler bodies for `init_pool`, `deposit`, `withdraw`, `request_advance` (worker-attested path), `claim_and_settle` (agent-cranked path), `liquidate`, and the reputation/oracle write paths can all be implemented. The ed25519 introspection helper, MPL Agent Registry owner-or-delegate verification, and Squads CPI verification are the three load-bearing helpers worth writing in `credmesh-shared` first.

## What's still open (intentionally not v1)

- Permissionless `claim_and_settle` cranking — needs payer-pre-auth pattern; deferred.
- Plain-EOA agents — Squads-only for v1.
- ML-derived credit curve.
- Mobile Wallet Adapter / Solana Mobile.
- Light Protocol compressed PDAs.
- Hyperliquid Lazer publisher.
