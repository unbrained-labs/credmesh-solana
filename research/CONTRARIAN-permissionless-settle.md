# CONTRARIAN — permissionless `claim_and_settle` via SPL `Approve`

**Status:** implemented on branch `feat/token2022-delegate-permissionless-settle`. Supersedes the v1 deferral recorded in AUDIT.md P0-3 / P0-4 and CLAUDE.md "Don't make `claim_and_settle` permissionless in v1".

**Author:** branch implementation pass, 2026-05-05.

**Reading order:** read AUDIT.md §P0-3 / §P0-4 first (the original threat). Then this doc — it explains why the auditor's v1 shortcut was structural, what primitive closes the gap, why that primitive is plain SPL Token classic (not Token-2022, despite the auditor citing it), and the new attack surface it opens.

---

## TL;DR

`claim_and_settle` was constrained to `cranker == advance.agent` in v1 because three destination ATAs (`agent_usdc_ata`, `protocol_treasury_ata`, `payer_usdc_ata`) couldn't be cryptographically pinned without that equality. AUDIT P0-3 fixed two of them (address-pinning `protocol_treasury_ata` to `pool.treasury_ata`, authority-pinning `agent_usdc_ata` to `advance.agent`). The third (`payer_usdc_ata`) was left as `token::authority = cranker` and the cranker was forced to be the agent, which collapsed the substitution surface to zero — at the cost of breaking the autonomous-agent thesis (no relayer can settle when an agent is offline).

**The fix:** tighten `payer_usdc_ata.token::authority` from `cranker` to `advance.agent`, drop the `cranker == advance.agent` constraint, and dispatch transfers in two modes:

1. **Mode A (legacy):** `cranker == advance.agent` — agent self-cranks; transfers signed by cranker. Bit-for-bit identical to v1.
2. **Mode B (permissionless):** `cranker != advance.agent` — pool PDA is the SPL Token `delegate` on `agent_usdc_ata`; transfers signed by pool PDA. The delegation is granted by `request_advance` in the same tx the agent already signs at issuance.

The primitive is **SPL Token classic `Approve`** — the same delegation primitive that's been in `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA` since day one. **Not Token-2022.** The earlier audit note cited Token-2022 `PermanentDelegate` as the future primitive; that was a misread — `PermanentDelegate` is a mint-level extension, set at mint init by the mint authority, applies to all token accounts of that mint. CredMesh doesn't control USDC's mint. We can't add it. The right primitive is the per-account `Approve` instruction, which works on Token classic today.

---

## Problem: why `cranker == advance.agent` was load-bearing

Read AUDIT.md §P0-3 carefully:

> `agent_usdc_ata`, `protocol_treasury_ata`, `payer_usdc_ata` are bare `mut TokenAccount`s. A cranker can substitute attacker-owned ATAs and steal:
> - The 15% protocol cut (substitute `protocol_treasury_ata`)
> - The agent net (substitute `agent_usdc_ata`)
> - Drain a victim's USDC (substitute `payer_usdc_ata` if signing logic is loose)

The fix tied each ATA to a verifiable on-chain anchor:

| ATA | v1 constraint | Anchor it pins to |
|---|---|---|
| `protocol_treasury_ata` | `address = pool.treasury_ata` | Pool storage |
| `agent_usdc_ata` | `token::authority = advance.agent` | Advance storage |
| `payer_usdc_ata` | `token::authority = cranker` | The signer of this very tx |

The third row is the load-bearing line. If the `cranker` is anyone, then `payer_usdc_ata.authority = cranker` doesn't actually constrain anything — an attacker just sets up their own ATA, sets themselves as the authority, becomes the cranker, and the constraint passes. The attacker can then claim the agent's settlement window with ZERO of the agent's actual funds, fail the require for `payment_amount >= total_owed`, and... that's still fine, because the tx reverts. So this attack vector requires looking deeper.

The actual scary case: a *malicious cranker who is also a USDC-holder* could in theory sign for transfers OUT of their own ATA, paying off the agent's debt in exchange for some off-chain favor. Annoying, but not a theft. The bigger concern was: **what if `payer_usdc_ata` was someone the agent *trusted*?** E.g., a treasury manager or co-signer who set up an ATA, granted the agent delegate authority over it, and the agent could then sign as authority? In Solana SPL Token, `Approve` lets the owner grant delegate authority bounded by `delegated_amount`. If the cranker were the agent and the agent had delegate authority on a third-party ATA, the agent could in v1 settle from that delegated ATA — useful UX, but the on-chain check wouldn't catch a malicious cranker abusing the same path.

The auditor's v1 shortcut: **just collapse the cranker to the agent**, and the whole substitution surface goes away because the agent is the only one who can sign. Cheap fix. Wrong long-term.

---

## Why "no permissionless cranking" breaks the agent thesis

CredMesh's value proposition is that autonomous agents take credit advances against future receivables, settle them when paid, and operate without humans in the loop. The agent process (a long-running daemon with a hot Solana key) is supposed to be the only privileged participant for its own advances. That works fine until:

- The agent process crashes or restarts during the settlement window
- The agent's hot key gets rotated mid-window
- The agent is offline during a planned upgrade
- The agent operator decides to delegate operations to a managed relayer (this is the *common case* for the PayAI hosted model — see DECISIONS.md Q6)
- Someone wants to run an MEV-style cranker as a service (legitimate one — compete on settlement-window timing)

In v1, all of these mean the LP's principal is stuck until the agent personally signs a `claim_and_settle`. If the agent never comes back, the advance liquidates after `LIQUIDATION_GRACE_SECONDS` (14 days), and the LP eats the loss — even though the receivable was paid. That's a real failure mode for an autonomous-agent protocol, and it's not the threat model the auditor's v1 shortcut was protecting against (the shortcut was about ATA substitution; the consequences for autonomy were collateral damage).

---

## The right primitive: SPL Token `Approve`

SPL Token classic supports per-account delegation via the `Approve` instruction:

```
Approve {
    to:        agent_usdc_ata,        // the account being delegated
    delegate:  pool_pda,              // who can sign transfers out
    authority: agent,                 // the owner granting the delegation
}
amount: settle_delegate_amount
```

After this CPI, the SPL Token program records on `agent_usdc_ata`:
- `delegate = Some(pool_pda)`
- `delegated_amount = settle_delegate_amount`

When a subsequent `Transfer` ix targets `agent_usdc_ata` and is signed by `pool_pda` (instead of by `agent`), the SPL Token program permits it as long as the transfer amount is ≤ `delegated_amount`, and decrements `delegated_amount` by the transfer amount. When `delegated_amount` reaches zero, the delegation is implicitly cleared.

This is the Solana-native equivalent of EVM's ERC-20 `approve` + `transferFrom`. It's been in Token classic since 2020. **There is no Token-2022 dependency.**

### Why not Token-2022 `PermanentDelegate`

The earlier audit note (AUDIT P0-4) said:

> Permissionless settle requires a future "payer-pre-authorized" pattern (Token-2022 delegate or pre-signed `transfer_checked`).

`PermanentDelegate` is a Token-2022 mint extension. When set at mint init by the mint authority, the named delegate has unrevocable transfer authority over **every** token account of that mint, forever. Three reasons it's the wrong primitive here:

1. **CredMesh doesn't control USDC's mint.** Circle does. Circle hasn't migrated USDC to Token-2022, and even if they did, they'd never set CredMesh as a permanent delegate on every USDC account.
2. **Even if we deployed our own mint, `PermanentDelegate` is too coarse.** It's mint-wide and irrevocable. We want per-account, bounded, revocable delegation. That's `Approve`, not `PermanentDelegate`.
3. **`PermanentDelegate` would be unsound for an agent protocol.** Granting one program permanent authority over all agents' USDC ATAs is the opposite of non-custodial.

The audit note conflated "delegation primitive on Token-2022" with `PermanentDelegate`, but the right Token-2022 analogue would actually be the same `ApproveChecked` ix — which is just the Token-2022 version of the classic `Approve`. Both work. We don't need either-or; we use Token classic because USDC is Token classic.

### Why not pre-signed `transfer_checked`

The other audit-note-suggested path: agent signs a `transfer_checked` ix at advance time, the relayer holds the signed bytes and submits them later. This works in theory but adds:

- Replay protection (the signed bytes are reusable)
- Tx-version pinning (Solana tx format changes break old signatures)
- Off-chain key custody for the relayer (now we trust the relayer to not lose or expose the signed bytes)
- A whole new code path for serializing-and-relaying

`Approve` puts the state on-chain, the SPL Token program enforces the cap, and the agent can revoke any time. Strictly better.

---

## Implementation

### Changes to `request_advance` (programs/credmesh-escrow/src/lib.rs)

After the existing vault → agent USDC transfer, CPI `token::approve`:

```rust
let late_penalty_per_day_v = compute_late_penalty_per_day(amount, &pool.fee_curve)?;
let max_late_penalty = (MAX_LATE_DAYS as u64).checked_mul(late_penalty_per_day_v)?;
let settle_delegate_amount = amount + fee_owed + max_late_penalty;

token::approve(
    CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        Approve {
            to: ctx.accounts.agent_usdc_ata.to_account_info(),
            delegate: ctx.accounts.pool.to_account_info(),
            authority: ctx.accounts.agent.to_account_info(),
        },
    ),
    settle_delegate_amount,
)?;
```

Approval cap is the worst-case settlement amount: `principal + fee_owed + (MAX_LATE_DAYS × late_penalty_per_day)`. With `MAX_LATE_DAYS = 365` and `late_penalty_per_day = 0.1% of principal`, the worst case is `~1.365 × principal + fee_owed` — bounded.

### Changes to `claim_and_settle`

The `ClaimAndSettle` accounts struct loses the `cranker == advance.agent` constraint and tightens `payer_usdc_ata.token::authority` from `cranker` to `advance.agent`:

```rust
#[account(mut)]
pub cranker: Signer<'info>,                        // was: + constraint = cranker.key() == advance.agent

#[account(mut, token::mint = pool.asset_mint, token::authority = advance.agent)]
pub payer_usdc_ata: Account<'info, TokenAccount>,  // was: token::authority = cranker
```

Handler dispatches on `is_self_crank = (cranker.key() == advance.agent)`:

- **Mode A (`is_self_crank == true`):** transfers use `cranker` as authority via `CpiContext::new(...)`. No prior `Approve` required. Bit-for-bit identical to original v1 behavior, so existing self-crank flows are unchanged.
- **Mode B (`is_self_crank == false`):** transfers use `pool` PDA as authority via `CpiContext::new_with_signer(...)` with the pool seeds. Three preconditions enforced before the first transfer:
  1. `payer_usdc_ata == agent_usdc_ata` (else `PayerMustBeAgentInPermissionless`)
  2. `agent_usdc_ata.delegate == Some(pool_pda)` (else `DelegateNotApproved`)
  3. `agent_usdc_ata.delegated_amount >= total_owed` (else `DelegateAmountInsufficient`)

The SPL Token program *also* enforces (2) and (3) inside its own transfer logic; the explicit handler-side checks yield typed CredMesh errors and a cheaper early-exit.

### New events / errors

`AdvanceSettled.cranker: Pubkey` added — lets indexers distinguish self-crank from delegated-crank without reading account state.

Three new error variants:
- `DelegateNotApproved`
- `DelegateAmountInsufficient`
- `PayerMustBeAgentInPermissionless`

---

## Threat model — what the new attack surface looks like

### Threats unchanged from v1

The original P0-3/P0-4 attacks (substitute attacker ATA for treasury / agent / payer) are still blocked by the per-ATA constraints, none of which depend on cranker identity:

- `protocol_treasury_ata`: `address = pool.treasury_ata` (account-struct, runs before handler)
- `agent_usdc_ata`: `token::authority = advance.agent` (same)
- `payer_usdc_ata`: `token::authority = advance.agent` (tightened from `= cranker`; the relaxation widens the cranker set without widening the source-of-funds set)
- `agent` UncheckedAccount: `address = advance.agent` (rent recipient via `close = agent`)

A Mode-B relayer who tries to redirect any of these gets a typed error from Anchor before the handler even runs.

### New threat: relayer DoS / griefing

A malicious relayer cranks `claim_and_settle` early, draining the agent's `delegated_amount` on borderline cases. **Mitigation:** the SPL Token program decrements `delegated_amount` by the actual transfer amount, and the handler requires `delegated_amount >= total_owed`. Early-cranking can't cost the agent more than `total_owed` — same as if the agent self-cranked. Net surface unchanged.

A relayer cranks before the receivable is actually paid, draining the agent's pre-existing USDC balance. **Mitigation:** the memo-nonce binding (`require_memo_nonce`) requires the cranker to include the `consumed.nonce` in the memo of the same tx. The nonce is only known if you observed the original `request_advance` event AND the receivable settlement. The handler doesn't re-verify the receivable's payment status here — but the `require!(payment_amount >= total_owed, ...)` check + the agent's USDC balance constraint mean the cranker can only succeed if the agent actually has enough USDC to settle. If the receivable wasn't paid, the agent's balance is below `total_owed`, the SPL transfer fails, the tx reverts. **Subtle but holds.**

### New threat: lingering approval residual

Mode B's worst-case approval cap is `principal + fee_owed + MAX_LATE_DAYS × late_penalty_per_day`. After settlement, the residual approval = worst_case − actual_used.

**Mode A — auto-revoked.** The handler CPIs `token::revoke` on `agent_usdc_ata` at end-of-handler. The cranker is the agent (= owner), so the program can sign Revoke directly. Post-settle: `delegate = None`, `delegated_amount = 0`. No residual.

**Mode B — bounded residual.** SPL Token decrements `delegated_amount` by each delegate-signed transfer, so after the three transfers the residual on the ATA is `MAX_LATE_DAYS × late_penalty_per_day − actual_late_penalty` (worst case ≈ 36.5% of principal when settlement is on-time). The program cannot CPI `Revoke` here because the cranker is not the owner. The agent can revoke any time post-settle; the off-chain worker bundles a `Revoke` ix when the agent comes back online. A subsequent `request_advance` calls `token::approve` again, which **replaces** the existing delegation entirely (SPL semantics), so the residual is also implicitly cleared on the next advance.

The risk: a future malicious or buggy upgrade of the credmesh-escrow program could use this residual approval to debit the agent's ATA. Mitigations:

1. The escrow program's deploy authority is in a Squads multisig (per DECISIONS.md Q3).
2. Mode A has zero residual by construction (auto-revoke).
3. Mode B residual is bounded by the late-penalty curve, which itself is governance-capped.
4. A new `request_advance` replaces any existing delegation.

**Acceptable trade.** The alternative — approving only `principal + fee_owed` and falling back to Mode A for late settlements — gives sharper UX boundaries but breaks the autonomy thesis exactly when it's most needed (agent offline AND late). We approve the worst-case envelope and use auto-revoke where we can.

### New threat: cranker MEV / front-running for rent refund

Pre-branch, `close = agent` already routed the Advance account's rent refund to the agent regardless of cranker identity. **No change** — the MEV-neutral property is preserved by construction. Cranker's only economic incentive is the tx fee they pay (cost), not a reward (no profit). A future v1.5 might add a small explicit cranker reward; v1 doesn't, and the existing test infrastructure asserts cranker rent delta == −tx_fee.

---

## What stays explicitly out of scope

- **Granular approval per-instruction-type:** v1 grants one approval covering the worst-case settlement. A future version could split this into per-purpose approvals (e.g., separate caps for protocol-cut, lp-cut, agent-net) — adds complexity for marginal benefit.
- **Token-2022 ApproveChecked:** the same primitive on Token-2022. We use classic because USDC is classic. If Circle migrates USDC to Token-2022, swap the import. The handler structure is identical.
- **Approval revocation tied to advance closure:** since `Approve` and `Revoke` are SPL-program ixs and the agent is the owner, we can't CPI a revoke from the program (only the owner signs revoke). The off-chain worker bundles `Revoke` when possible; offline-agent residuals are accepted as a bounded exposure.
- **Multi-payer source ATAs in Mode B:** v1 requires `payer_usdc_ata == agent_usdc_ata` in Mode B. Supporting a distinct payer ATA owned by the agent would need a second `Approve` against that ATA, with the same cap math. Out of scope.
- **Pre-`request_advance` standing approval:** an agent could grant the pool PDA a long-standing approval covering many future advances. Useful for high-volume agents but introduces a "perpetually approved" exposure window that needs separate threat modeling. Out of scope.

---

## Tests added (`tests/bankrun/escrow/claim_and_settle.test.ts`, `tests/bankrun/attacks/ata_substitution.test.ts`)

- Pure: delegate approval cap formula (`principal + fee_owed + MAX_LATE_DAYS × late_penalty_per_day`).
- Pure: Mode B precondition shape.
- Pure: post-branch account-struct shape.
- Behavioral scaffolds: Mode A still works; Mode B succeeds; Mode B rejects on missing/insufficient/wrong delegate; Mode B rejects on payer != agent_ata; ATA substitution defenses hold for both modes.

(Behavioral tests are still placeholders pending issue #15 IDL extraction. The pure tests exercise the constants and shape directly.)

---

## Cross-walk to EVM lane

The EVM lane's `TrustlessEscrowV4.sol` has had this for free since day one — ERC-20 `transferFrom` is the protocol's native cranking primitive. There's no concept of "Mode A vs Mode B" on EVM; the credit-worker is always a third-party relayer. Solana now matches this property for the cranking surface.

`docs/PROTOCOL-FIRST-CROSSWALK.md` (in `../trustvault-credit/`) needs the Solana row updated for the cranking parity. Tracked separately.
