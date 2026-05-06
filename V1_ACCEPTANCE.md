# V1 Acceptance criteria

What "v1 ready to ship" means. Drives sprint planning; updated as scope shifts.

Last refresh: 2026-05-06 — post-EVM-bridge pivot.

> **Architecture note:** v1 ships the EVM-as-source-of-truth model. EVM holds
> identity, reputation, and the canonical outstanding-balance ledger. Solana
> holds the LP vault, advance issuance, and settlement, gated by short-TTL
> ed25519 credit attestations from a whitelisted bridge signer. See
> `BRUTAL-TRUTH-EVM-PARITY-DRIFT.md` § "Pivot to EVM-as-source-of-truth" for
> rationale.

## On-chain

- [-] `credmesh-escrow` deployed on devnet with verified-build hash
  *(handlers complete + cargo check clean; deploy keypair reserved at
  `DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF`)*
  - [x] `init_pool` creates Pool + share mint + vault ATA + mints virtual-shares dead supply
  - [x] `deposit` mints shares using u128 virtual-shares math; first-depositor inflation cost ≥ 10⁶× attacker profit
  - [x] `withdraw` enforces idle-only cap; fails atomically when deployed > idle
  - [x] `request_advance` consumes ed25519 credit attestation: prior-ix verify
        (sysvar introspection), signer ∈ AllowedSigner registry with
        kind=CreditBridge, message offsets/version asserted, freshness
        ≤ 15 min, agent + pool match, underwrites against
        `attested_credit_limit − attested_outstanding`, opens permanent
        `ConsumedPayment` PDA
  - [x] `claim_and_settle` (single-mode, agent self-settles) computes 3-tranche
        waterfall with checked math; sum invariant holds; remainder rounds to
        agent; events emit; memo nonce binding preserved
  - [x] `liquidate` marks `Advance.state = Liquidated`, decrements
        `Pool.deployed_amount`, applies pool-loss surcharge; `ConsumedPayment`
        permanence preserved (AUDIT P0-5)
  - [x] `propose_params` / `execute_params` enforce timelock; FeeCurve
        invariants validated at propose-time
  - [x] Squads CPI verification on `propose_params` / `skim_protocol_fees` —
        governance is `UncheckedAccount` address-pinned to `pool.governance`
        with `require_squads_governance_cpi` introspection
- [x] `credmesh-attestor-registry` (renamed from `credmesh-receivable-oracle`)
      — governance-controlled `AllowedSigner` PDA whitelist with kind tags
  - [x] `init_registry(governance)` — rejects `Pubkey::default()`
  - [x] `add_allowed_signer(signer, kind)` — Squads CPI gated
  - [x] `remove_allowed_signer()` — Squads CPI gated, close = rent refund
  - [x] `set_governance(new_governance)` — Squads CPI gated; rejects
        `Pubkey::default()`
- [x] **Squads CPI gate tightened**: `require_squads_governance_cpi`
      additionally requires the vault PDA to appear as **writable** in
      the Squads ix's account list. Narrows from "any Squads ix
      mentioning the vault" to "Squads ix actually authorizing-and-
      spending against the vault"
- [x] **Per-agent rolling-window issuance cap on-chain**:
      `Pool.agent_window_cap` + `AgentIssuanceLedger` PDA
      (`(pool, agent)` keyed). Rolls over `AGENT_WINDOW_SECONDS = 24h`;
      bridge-key-compromise blast-radius bound below the 15-min TTL.
      `0` disables (devnet bring-up); mainnet config MUST be > 0
- [x] **Cross-cluster ed25519 attestation replay closed**: `Pool.chain_id`
      set at init; `request_advance` asserts `msg.chain_id ==
      pool.chain_id`
- [x] `credmesh-reputation` deleted (EVM is canonical)
- [x] All cross-program reads verify owner pubkey + re-derive PDA + check
      8-byte discriminator + typed deserialize via Anchor 0.30 `Account<T>`

## Off-chain

- [x] `ts/server` — Hono backend serving `/.well-known/agent.json` (cached at
      module load) and SIWS `/auth/nonce`
- [x] `ts/bridge` — EVM ⇒ Solana credit-attestation bridge
  - [x] `POST /quote` reads EVM `ReputationCreditOracle.maxExposure(agent)`
        and `TrustlessEscrow.exposure(agent)` via viem, encodes the canonical
        128-byte `ed25519_credit_message`, signs with the bridge's ed25519
        secret key, returns `{message_b64, signature_b64, signer_pubkey_b58,
        expires_at, attested_at, credit_limit_atoms, outstanding_atoms}`
  - [x] Bridge signing key loaded from a Solana-keypair-format JSON file
        (64 bytes secret + public)
  - [x] Service refuses to start if any required env var is missing —
        explicit refusal beats silent fallback
  - [x] Bearer-token auth (`BRIDGE_AUTH_TOKENS`) + per-key token-bucket
        rate limit (default 30-burst / 12-per-min steady). Token-less
        mode (devnet bring-up) rate-limits by remote IP
  - [x] Solana event tail — subscribes via `logsNotifications`, decodes
        AdvanceIssued/Settled/Liquidated by Anchor `event:<Name>`
        discriminator, forwards to `<EVM_CREDIT_WORKER_URL>/solana-event`
        when configured (log-only mode otherwise). The EVM-side payload
        schema versioning is independent
- [x] `ts/keeper` — liquidation crank for advances past
      `expires_at + 14 days`
- [x] Bridge signer whitelist enforced on-chain (no `oracle_worker_authority`
      / `reputation_writer_authority` left to compromise)

## Tests

- [x] Pure-math `#[cfg(test)]` suite in `programs/credmesh-escrow/src/
      pricing.rs`: 16 tests covering waterfall ceiling, fee monotonicity
      (principal/duration/utilization), risk-premium clamping, late-
      penalty surcharge, virtual-shares first-depositor inflation
      defense, share-price monotonicity under yield, redeem round-trip,
      and utilization at all bounds. Run via `cargo test --workspace --lib`
      or `npm test`
- [x] Bridge + shared + server + keeper TS typecheck clean
- [ ] Bankrun behavioural attack fixtures (cross-agent ed25519 replay,
      `ConsumedPayment` close-then-reinit, ATA substitution on
      `claim_and_settle`, sysvar instructions spoofing). Deferred —
      pure-math + on-chain account-constraint defenses cover the
      enforcement; bankrun reproductions are a v1.5 hardening item
- [ ] Devnet end-to-end: full advance lifecycle (deposit → quote →
      request_advance → settle → withdraw) with real Circle USDC + the
      bridge service

## Audit + governance

- [x] Internal multi-pass audit on `credmesh-escrow` + `credmesh-attestor-
      registry` (4 Claude code-reviewers + Kimi K2 independent-model audit;
      all P0/P1 findings addressed)
- ~~External independent audit firm engagement~~ — explicitly out of scope
  for v1. The internal multi-pass audit + iterated /security-review +
  /simplify + /code-maturity-assessor passes are the deployed-state
  validation surface. Re-flag as a v1.5+ item if growth justifies the
  spend.
- [ ] Squads v4 multisig deployed for protocol governance with timelock
- [ ] All program upgrade authorities transferred to Squads vault
- [x] Verified-build commit hashes published for prior-state programs;
      republish required after the pivot's structural changes

## Documentation

- [x] CLAUDE.md — updated for the EVM-bridge architecture
- [x] BRUTAL-TRUTH-EVM-PARITY-DRIFT.md — pivot rationale appended
- [x] DECISIONS.md — Q3, Q4, Q9, Q11, Q13 amended for the pivot
- [x] AUDIT.md — pivot impact noted
- [x] DEPLOYMENT.md — Docker recipe + key rotation procedure
- [x] CONTRIBUTING.md
- [x] docs/ARCHITECTURE.md — program structure + PDAs (Mermaid)
- [x] docs/LOGIC_FLOW.md — sequence diagrams + invariants table
- [x] ts/bridge/README.md — env vars, trust model, what works vs pending
- [ ] Public docs site for: agent onboarding (via EVM → bridge → Solana),
      LP onboarding, governance procedures

## Mainnet readiness gates

Each must be green before mainnet flip:

1. [ ] Devnet end-to-end exercised with the bridge live (≥ 100 advances
       issued + settled against EVM-attested limits) — operational
2. [x] Audit findings all resolved or accepted with documented rationale
       (internal multi-pass + repeated /security-review /simplify
       /code-maturity-assessor sweeps; external audit explicitly out
       of scope)
3. [ ] Squads governance multisig configured (members, threshold,
       timelock) — operational
4. [ ] Bridge ed25519 signer rotated at least once on devnet (proves the
       `add_allowed_signer` / `remove_allowed_signer` flow works); both
       scripts shipped (`scripts/init_registry.ts` + `scripts/add_allowed_signer.ts`)
5. [ ] Hard caps active: `max_advance_pct_bps = 3000`,
       `max_advance_abs = 100_000_000` (= $100), `agent_window_cap > 0`
6. [ ] Insurance buffer: protocol treasury seeded with at least 5% of
       expected vault TVL — operational
7. [ ] EVM-side `ReputationCreditOracle` + `TrustlessEscrow` deployed on
       the paired EVM mainnet (Base) at the addresses the bridge is
       configured to read — EVM-lane prereq
8. [ ] `BRIDGE_AUTH_TOKENS` set in production env (no token-less mode)
9. [ ] Bridge signing key migrated off filesystem to HSM/KMS — v1.5
       hardening; doc the operational runbook for the v1 ship

## v1 explicitly NOT in scope (deferred)

- Solana-native reputation scoring — EVM is canonical
- Marketplace / receivable primitives on Solana
- ML-derived credit curves
- Mobile Wallet Adapter / Solana Mobile
- Hyperliquid Lazer publisher
- Light Protocol compressed PDAs
- Multi-asset pools (USDC only)
- Per-instruction-type timelock granularity
- Token-2022 USDC handling (Circle hasn't migrated)
- Embedded-wallet (Phantom Portal) auth
- Permissionless `claim_and_settle` cranking — reverted in the EVM-bridge
  pivot
- Multi-issuer SAS attestations (deferred to v1.5; schema documented now)
- Bridge-signer quorum (any-valid-sig in v1; quorum is v1.5 hardening)

## Legend

- `[x]` complete
- `[-]` partial / in flight
- `[ ]` not started
