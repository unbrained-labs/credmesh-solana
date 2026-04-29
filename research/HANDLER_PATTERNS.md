# HANDLER_PATTERNS — Canonical Solana lending-protocol patterns CredMesh lifts

Synthesized from production, audited Anchor codebases at the commit hashes verified at fetch time:
- MarginFi v2 (`mrgnlabs/marginfi-v2`): `843aa82df852b9e9a3c555e67ffd12aa53f4805b`
- Solend SPL fork (`solendprotocol/solana-program-library`, branch `mainnet`): `d04ce00bbf4356c4fd32b3be38eb9760b696bb3e`
- Kamino klend (`Kamino-Finance/klend`): `95d694b0bd0593ed354aea0e882f185329d656c4`
- Drift v2 (`drift-labs/protocol-v2`): `0ae3e3b1db782a6765c3525b3dec38ad4d9d3a62`
- Squads v4 (`Squads-Protocol/v4`, governance pattern): branch `main`

This document is the **handler-implementation reference manual**. Each pattern below is byte-for-byte from the cited source at the cited commit. Where CredMesh handler bodies in `programs/credmesh-escrow/src/lib.rs` adapt these patterns, AUDIT.md notes the lift point.

---

## Pattern 1 — Vault deposit/redeem with cToken (exchange-rate) math

**Source**: `solendprotocol/solana-program-library/token-lending/sdk/src/state/reserve.rs` lines 875-930. Mirror in `Kamino-Finance/klend/programs/klend/src/state/reserve.rs` lines 1272-1430.

```rust
fn exchange_rate(&self, total_liquidity: Decimal)
    -> Result<CollateralExchangeRate, ProgramError>
{
    let rate = if self.mint_total_supply == 0 || total_liquidity == Decimal::zero() {
        Rate::from_scaled_val(INITIAL_COLLATERAL_RATE)   // virtual offset sentinel
    } else {
        Rate::try_from(Decimal::from(self.mint_total_supply).try_div(total_liquidity)?)?
    };
    Ok(CollateralExchangeRate(rate))
}
```

Kamino's u128/U256 widening for cast→mul→div→cast (klend reserve.rs 1318-1336):

```rust
pub fn collateral_to_liquidity_ceil(&self, collateral_amount: u64) -> u64 {
    let collateral_amount_u256 = U256::from(collateral_amount);
    let liquidity_sbf = BigFraction::from(self.liquidity).0;
    let collateral_supply_u256 = U256::from(self.collateral_supply);
    let liquidity_ceil_sbf = collateral_amount_u256
        .checked_mul(liquidity_sbf)
        .and_then(|res| res.checked_add(collateral_supply_u256 - U256::one()))   // ceil
        .and_then(|res| res.checked_div(collateral_supply_u256))
        .expect("overflow");
    Fraction::try_from(BigFraction(liquidity_ceil_sbf)).expect("overflow on conversion").to_ceil()
}
```

**Asymmetric rounding**: Solend & Kamino floor on deposits and ceil on withdrawals so the protocol always wins the dust.

**CredMesh adapt**: u128 covers ~$18T USDC (6 decimals). Fine for v1. If v2 multi-asset pools introduce 18-decimal tokens, switch intermediate math to U256.

---

## Pattern 2 — SPL token transfer via PDA-signer

**Source**: `mrgnlabs/marginfi-v2/programs/marginfi/src/macros.rs` 116-124; `state/bank.rs` 690-738.

```rust
#[macro_export] macro_rules! bank_signer {
    ($vault_type: expr, $bank_pk: expr, $authority_bump: expr) => {
        &[&[
            $vault_type.get_authority_seed().as_ref(),
            &$bank_pk.to_bytes(),
            &[$authority_bump],
        ]]
    };
}
```

CredMesh equivalent: `[POOL_SEED, asset_mint.as_ref(), &[bump]]`. Always cache `bump` in account state — `find_program_address` in a hot path costs ~1500 CU.

---

## Pattern 3 — Multi-tranche waterfall in a single instruction

**Source**: `mrgnlabs/marginfi-v2/programs/marginfi/src/instructions/marginfi_group/handle_bankruptcy.rs` lines 126-220.

Compute all cuts up-front using checked u128 math. Round DOWN for protocol & LP cuts, agent receives the remainder. Three sequential `transfer_checked` CPIs. **One event emitted at the end** — never between CPIs.

```rust
let covered_by_insurance = min(bad_debt, available_insurance_fund);
let socialized_loss = max(bad_debt - covered_by_insurance, I80F48::ZERO);
// CPI 1
bank.withdraw_spl_transfer(covered_amount, insurance_vault, liquidity_vault, ...)?;
// State mutation 2 (no CPI)
bank.socialize_loss(socialized_loss)?;
// State mutation 3 (no CPI)
BankAccountWrapper::find(...).repay(bad_debt)?;
emit!(LendingPoolBankHandleBankruptcyEvent { /* full event last */ });
```

**CredMesh adapt**: `claim_and_settle` distributes `payment_amount` as `protocol_cut → lp_cut → agent_net`. Three CPIs (or skip one if agent is also payer source). Single `AdvanceSettled` event at the end.

---

## Pattern 4 — Cross-program account read

Two paths.

**Path A** — Anchor `AccountLoader<'info, T>` when types are shared:

```rust
#[account(
    mut,
    constraint = {
        let user = integration_acc_2.load()?;
        user.validate_spot_position(...).is_ok()
    } @ MarginfiError::DriftInvalidSpotPositions
)]
pub integration_acc_2: AccountLoader<'info, MinimalUser>,
```

**Path B** — Manual three-step (Drift `signed_msg_user.rs` 176-200):

```rust
fn load(&self) -> DriftResult<SignedMsgUserOrdersZeroCopy> {
    validate!(self.owner == &ID, ErrorCode::DefaultError, "invalid owner")?;
    let data = self.try_borrow_data()?;
    let (discriminator, data) = Ref::map_split(data, |d| d.split_at(8));
    validate!(*discriminator == SignedMsgUserOrders::discriminator(),
              ErrorCode::DefaultError, "invalid discriminator")?;
    let (fixed, data) = Ref::map_split(data, |d| d.split_at(40));
    Ok(SignedMsgUserOrdersZeroCopy { fixed: Ref::map(fixed, |b| bytemuck::from_bytes(b)), data })
}
```

**CredMesh adapt**: `credmesh-shared::cross_program::read_cross_program_account<T>` already implements Path B (owner → address → discriminator → typed deserialize).

---

## Pattern 5 — Instruction sysvar introspection (ed25519 + memo)

**Source**: `drift-labs/protocol-v2/programs/drift/src/validation/sig_verification.rs` lines 148-313.

The asymmetric.re/Relay-class fix at lines 260-265 is critical:

```rust
if offsets.signature_instruction_index != current_ix_index
    || offsets.public_key_instruction_index != current_ix_index
    || offsets.message_instruction_index != current_ix_index
{
    return Err(SignatureVerificationError::InvalidInstructionIndex.into());
}
```

Without this, a malicious caller can put the ed25519 verify ix in slot 0 with offsets that point past your ix data into a memo ix containing the attacker's payload.

**CredMesh adapt**: `credmesh-shared::ix_introspection::verify_prev_ed25519` enforces this. Use `require_memo_nonce` for the claim_and_settle binding (scans all ixs for a memo with matching bytes).

---

## Pattern 6 — Anchor `init` PDA as permanent replay protection

**Source**: `drift-labs/protocol-v2/programs/drift/src/instructions/user.rs` 4475-4496.

```rust
#[account(
    init,
    seeds = [b"user", authority.key.as_ref(), sub_account_id.to_le_bytes().as_ref()],
    space = User::SIZE,
    bump,
    payer = payer
)]
pub user: AccountLoader<'info, User>,
```

`init` (not `init_if_needed`) compiles to `system_program::create_account` which fails if the PDA exists with non-zero data. **Never expose a close handler for a PDA whose entire purpose is replay protection.**

**CredMesh adapt**: `ConsumedPayment` uses `init`, no close path anywhere. AUDIT P0-5 fix.

---

## Pattern 7 — Permissionless cranking with rent-refund-to-victim

**Source**: `drift-labs/protocol-v2/programs/drift/src/instructions/keeper.rs` 371-427.

Drift pays the cranker a flat reward (a `filler` account); user PDAs are not closed by the cranker. CredMesh's analog: cranker reward paid from protocol fee bucket; rent on closed `Advance` PDA goes to `agent`, not cranker — neutralizes MEV.

**CredMesh adapt**: `claim_and_settle` closes `Advance` to `agent`. `Liquidate` keeps `Advance` alive (audit trail) and decrements `Pool` state. v1 has no cranker reward; CredMesh's off-chain keeper handles cranking. v2 may add a flat reward.

---

## Pattern 8 — Anchor constraint declaration order

Anchor evaluates constraints in *declaration order within `#[account(...)]`*. Canonical safe order:

```
mut → seeds + bump → has_one → constraint
```

Putting `constraint = bank.load()?.something == other.key()` BEFORE `has_one = other` creates a confused-deputy: you've trusted the bank's stored field to validate `other`, but if the bank was forged, you've routed through it.

**CredMesh adapt**: Always: `seeds + bump → has_one (or address) → custom constraint`.

---

## Pattern 9 — Squads-style time-locked governance

**Source**: `Squads-Protocol/v4/programs/squads_multisig_program/src/instructions/vault_transaction_execute.rs` 88-154.

Squads vault PDA seeds:

```rust
let vault_seeds = &[
    SEED_PREFIX,                                  // b"multisig"
    multisig_key.as_ref(),
    SEED_VAULT,                                   // b"vault"
    &transaction.vault_index.to_le_bytes(),
    &[transaction.vault_bump],
];
```

CredMesh verifies a Squads-vault signer:

```rust
let expected_vault = Pubkey::create_program_address(
    &[b"multisig", squads_multisig_key.as_ref(), b"vault",
      &0u8.to_le_bytes(), &[squads_vault_bump]],
    &SQUADS_PROGRAM_ID,
).map_err(|_| EscrowError::InvalidGovernanceSigner)?;
require_keys_eq!(ctx.accounts.governance_signer.key(), expected_vault,
                 EscrowError::InvalidGovernanceSigner);
```

**CredMesh adapt**: `propose_params` records `executable_at = now + timelock`. `execute_params` requires `now >= executable_at` and the Squads-vault signer check above.

---

## Pattern 10 — Compute units, errors, invariants, events

- Every math op uses `checked_*` + maps overflow to a typed program error.
- Cast u128→u64 only via `u64::try_from(...).map_err(|_| Err::MathOverflow)?`.
- Emit one event per handler, AS THE LAST STATEMENT.
- Run final invariants (`require!(...)`) on the post-state right before the event.
- Cache bumps in account state at init; never `find_program_address` in hot paths.

---

## Top 5 patterns CredMesh lifts verbatim

1. MarginFi's `bank_signer!` macro → CredMesh's pool-seed array, bump cached on Pool
2. Drift's `verify_and_decode_ed25519_msg` → ported into `credmesh-shared::ix_introspection`
3. Drift's `send_from_program_vault` → handles transfer_checked + Token-2022 forward-compat
4. Solend `CollateralExchangeRate` + Kamino U256 widening → preview_deposit / preview_redeem in escrow
5. MarginFi bankruptcy waterfall structure → claim_and_settle 3-tranche distribution

## Anti-patterns to avoid

- Solend SPL `processor.rs` style (manual `next_account_info`) — pre-Anchor, 3-5x CU cost
- `init_if_needed` for replay-protection PDAs — allows reinit after close
- Mid-CPI state mutation followed by a possible-revert
- `find_program_address` in hot paths
- Implicit `as u64` casts that truncate silently
- `emit!` before all CPIs succeed
- `transfer` (deprecated) — use `transfer_checked`

## Source URLs

- MarginFi: https://github.com/mrgnlabs/marginfi-v2
- Solend: https://github.com/solendprotocol/solana-program-library/tree/mainnet
- Kamino: https://github.com/Kamino-Finance/klend
- Drift: https://github.com/drift-labs/protocol-v2
- Squads v4: https://github.com/Squads-Protocol/v4
