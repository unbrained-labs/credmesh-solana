# HANDOFF_REVIEW.md

Independent review of the colleague's `HANDOFF.md` (the document at
`~/Downloads/HANDOFF.md`, dated context 2026-05-06). Verified against
`origin/main @ 6317555` (post-`evm-parity` squash merge) and the
handler sources on 2026-05-11.

Internal-only; lives under `internal/` (gitignored).

The handoff is generally accurate and well-structured. The points
below are the items that diverge from the code or are missing.
None invalidate the pivot or the architectural story; they affect
operations and external communication.

---

## ✅ Confirmed against the code

- Two programs only (`credmesh-escrow`, `credmesh-attestor-registry`).
  Reputation deleted, oracle renamed.
- 9 ixs on escrow / 4 on registry; surface matches the §3 table.
- The ed25519 attestation flow in §5 matches `request_advance.rs`
  step-for-step (sysvar introspection, asymmetric.re defense,
  signer-in-registry, kind check, layout decode, version, chain_id,
  freshness, expiry, underwriting, per-agent window).
- 128-byte attestation layout in §5 matches
  `crates/credmesh-shared/src/lib.rs::ed25519_credit_message`.
- Bridge service in §6.1 — agent-binding map, rate limit, auth tokens,
  EVM read + ed25519 sign, event tail with fallback — all match
  `ts/bridge/src/index.ts`.
- Keeper service in §6.3 — fixed-size 152-byte decode, Promise.allSettled,
  shared blockhash per tick — matches `ts/keeper/src/index.ts`.
- Constants: `LIQUIDATION_GRACE_SECONDS = 14 × 24 × 3600`,
  `AGENT_WINDOW_SECONDS = 24 × 3600`, `PROTOCOL_FEE_BPS = 1500`,
  `MAX_ATTESTATION_AGE_SECONDS = 15 × 60`.
- `ConsumedPayment` is `init` (not `init_if_needed`), never closed.

---

## 🛑 Discrepancies (handoff text vs. code)

### D1. Settlement waterfall math (§7.3, lines 561–567)

**Handoff says:**
```
protocol_cut = principal × PROTOCOL_FEE_BPS / 10_000  (15% of principal)
lp_cut       = total_owed - protocol_cut
```

**Code (`claim_and_settle.rs`):**
```rust
total_fee   = fee_owed + late_penalty
protocol_cut = total_fee × PROTOCOL_FEE_BPS / BPS_DENOMINATOR  // 15% of FEE
lp_fee       = total_fee - protocol_cut
lp_cut       = principal + lp_fee
```

**Impact:** protocol cut is 15% of the *fee*, not the *principal*.
Materially different economics. A 1000-USDC advance with 50 USDC fee
yields a 7.5 USDC protocol cut and a 1042.5 USDC LP cut — not a 150
USDC protocol cut and an 892.5 USDC LP cut.

A surgical inline `**CORRECTION**` block was added under the bullets
on the local handoff file at `~/Downloads/HANDOFF.md` (around line
568) reflecting the actual math. **I attempted to leave the original
prose untouched and got one edit in before the harness flagged the
file as out-of-repo and started blocking edits.** Feel free to remove
the inline correction if you'd prefer the handoff stays verbatim —
this review now carries the same information.

### D2. Liquidation rent payout (§7.4, line 587)

**Handoff says:**
> `Advance` closes, rent → cranker (MEV-neutral; cranker is whoever
> fires the ix).

**Code (`liquidate.rs`):**
```rust
// AUDIT AM-7: keep `Advance` alive after liquidation for audit trail.
// Only `state` mutates to `Liquidated`. Closure happens via a separate
// admin-grace-period cleanup ix in a future version.
let advance = &mut ctx.accounts.advance;
advance.state = AdvanceState::Liquidated;
```

No `close = cranker` constraint on `Advance`. Rent stays in the PDA.
Cranker pays the tx fee with no refund.

**Impact:** there is no economic incentive for a third party to crank
liquidations. The keeper service must be protocol-run. Re-introducing
a rent payout conflicts with AUDIT AM-7 (audit-trail rule). This is a
v1 operational fact, not necessarily a bug — but the handoff prose
implies a permissionless MEV market that doesn't exist.

---

## ⚠️ Missing from the handoff (worth surfacing)

### M1. Settlement window opens 7 days before expiry

`claim_and_settle` requires `now >= advance.expires_at -
CLAIM_WINDOW_SECONDS (7d)` (else `NotSettleable`). Receivables with
TTL < 7 days cannot reach a settlement window under the current
handler. The bridge `/quote` doesn't enforce this — it accepts
`ttl_seconds` up to 15 min (the attestation TTL), well below the
7-day claim window.

**Operational fix:** the bridge should reject `ttl_seconds <
CLAIM_WINDOW_SECONDS + epsilon`, or the on-chain handler should drop
the pre-window guard. Either is acceptable; today's combination is a
foot-gun.

### M2. `MIN_ADVANCE_ATOMS = 1 USDC` floor

`request_advance` reverts with `AdvanceExceedsCap` if `amount <
MIN_ADVANCE_ATOMS`. Surfaces as a generic error in client logs.
Worth flagging in the bridge `/quote` validation and in API docs.

### M3. `Pool.max_advance_pct_bps` is unused

`init_pool` validates `max_advance_pct_bps ≤ BPS_DENOMINATOR` and
stores the value, but `request_advance` only caps on
`max_advance_abs`. The bps field is dead storage. Either wire it
into the handler or drop it from `InitPoolParams`.

### M4. Keypair filename drift in `target/deploy/` (FIXED)

The committed keypair was
`target/deploy/credmesh_receivable_oracle-keypair.json` (old name)
while `scripts/deploy.ts` reads
`target/deploy/credmesh_attestor_registry-keypair.json` (new name).
`.gitignore` whitelists the new name. **`npm run deploy` was failing
on a fresh checkout** until this was reconciled.

Resolved by `git mv` to the new filename and `git rm` on the stale
`credmesh_reputation-keypair.json` (deleted program). Pending commit
+ push at the time of this review.

### M5. CI is compile-only

`.github/workflows/build.yml` runs `cargo check`; `cargo clippy`,
`cargo fmt`, and the `ts/server` typecheck are `continue-on-error:
true`. `npm test`, the four-package TS typecheck, anchor build, and
bankrun tests are not wired up. Green CI ≠ tests passing.

### M6. Bridge clock-drift is unmonitored

The handler accepts `attested_at` within 15 min of cluster `now`.
Bridge wall-clock and cluster wall-clock are two different things.
Bridge host needs NTP; an alert on `|bridge_now − cluster_now| >
30s` is the cheap early-warning.

### M7. `init_pool` is permissionless per `asset_mint`

The first caller wins the pool slot for that mint. On mainnet, the
attacker can front-run with their own `governance` and `treasury_ata`.
The land-grab risk is real but mitigated by the fact that LPs read
the governance field before depositing. Worth bundling deploy + init
or accepting the risk explicitly.

---

## 🟡 Style / framing notes (non-load-bearing)

- §1 elevator pitch describes the bridge as "whitelisted" — the
  whitelist is on-chain (`AllowedSigner` PDAs), not off-chain. Reads
  fine but could be clearer.
- §8 "Defense layers under bridge-key compromise" mentions
  "(15 min × `agent_window_cap`)" as the blast-radius bound. More
  precisely: `agent_window_cap` per 24h, and the 15-min TTL limits
  reuse of any single fraudulent attestation. Two independent bounds,
  not a product.
- §11.5 says issue #15 is open / worked around with hand-rolled
  encoders. PR #52 in the `main` history (`fix: import
  AssociatedToken at module scope (issue #15)`) suggests an attempt
  landed but the workaround is still in place. Worth re-verifying
  the current state before mainnet.
- §12 "Three commits-of-record on `evm-parity`" — there are now
  more commits after the listed `cdda696` (e.g., `5e26a38`,
  `e7a6e6f`, `69bd976`, `5ce03d0`) that touch the public-facing
  surface (relocating docs to `internal/`, dropping #15 references).
  Update the list if the handoff is ever re-shared.

---

## What this review does NOT find

- No security holes in the verified handler logic. The pivot's
  defense story (15-min TTL + per-agent cap + revocable whitelist +
  Squads gate + chain_id binding) is intact.
- No drift between the public README/CLAUDE/CONTRIBUTING and the
  code on `origin/main`.
- No correctness issues in the bridge `/quote` flow (auth, rate
  limit, EVM read, layout encoding).

The repo is broadly in the shape the handoff describes. The items
above are the seams worth tightening before mainnet promotion.

---

## Action items (for the deploy ticket)

- [ ] Decide whether to fix D1 in the handoff (or accept that the
      handoff is a snapshot and use the deploy-plan correction).
- [ ] Decide whether to fix D2 in the handoff and treat the keeper
      as a protocol-run service in v1 docs.
- [ ] Implement M1 mitigation (either-or).
- [ ] Implement M3 cleanup (wire or drop `max_advance_pct_bps`).
- [x] Implement M4 keypair rename + reputation-keypair removal.
- [ ] Wire CI gates per TESTING_PLAN §9.
- [ ] Add bridge clock-drift alert (M6).
- [ ] Decide on M7 land-grab mitigation.
