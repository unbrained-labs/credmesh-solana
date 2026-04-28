# 01 — Vault, Escrow & Credit Issuance Layer

**Scope**: On-chain primitive for the Solana equivalent of `TrustlessEscrowV3.sol` — pooled USDC vault, advance issuance, oracle composability, repayment waterfall, governance.

**Method**: Research agent invoked Helius skills (`jupiter`, `dflow`, `svm`) and Exa search. Compared Kamino, Solend, MarginFi, Jupiter Lend, Drift, Loopscale.

---

## Recommended on-chain architecture

Use a single Anchor program (`credmesh_escrow`) that owns one or more **Pool** PDAs. Each pool is a USDC vault with a dedicated SPL share-mint (the Solana analog of an ERC-4626 share token). Advances are tracked as per-agent **Advance** PDAs holding principal/fee/expiry; LPs get a wallet SPL token that auto-appreciates against `total_assets / total_shares`. Oracles are **separate programs** that the escrow CPIs into via a shared `IOracle` instruction interface (a small Anchor trait + discriminator). Governance is held by a Squads v4 multisig with a non-zero time-lock; admin keys cannot touch advance issuance because that path is fully on-chain-gated.

```
credmesh_escrow (program)
├── Pool PDA           seeds=["pool", asset_mint]
│   ├── usdc_vault_ata (token-account, authority=Pool PDA)
│   ├── share_mint     (SPL mint, mint_authority=Pool PDA)
│   ├── total_assets, total_shares, deployed_amount, accrued_protocol_fees
│   ├── governance: Pubkey  (Squads vault)
│   ├── pending_params, execute_after  (timelock)
│   └── oracle_registry: Vec<{oracle_program, ratio_bps}>
├── Advance PDA        seeds=["advance", agent, receivable_id]
│   ├── agent, oracle_program, receivable_id
│   ├── principal, fee, issued_at, expires_at, settled, liquidated
└── LpPosition         (just a normal Associated Token Account holding share_mint)
```

External programs (CPI-only):
- `reputation_oracle` (program) — exposes `get_credit(agent) -> u64`
- `receivable_oracle` (program) — exposes `verify_receivable(agent, id) -> ReceivableData { amount, expires_at, ... }`
- Squads v4 multisig — owns program upgrade authority + Pool.governance

## Vault primitive choice

**Recommendation: SPL-Token share-mint with PDA mint-authority and an exchange-rate model** (the canonical Kamino/Solend/Jupiter Lend pattern). Treat shares like cTokens/jlTokens: amount in user wallet stays constant, value rises as `total_assets` grows from settled fees.

Alternatives considered:
1. **Token-2022 with InterestBearingMint extension** — looks elegant but bakes a fixed APY into the mint and doesn't reflect actual realized fees from the waterfall. Plus most Solana DeFi infra still treats Token-2022 as a second-class citizen, and transfer-hook/transfer-fee compatibility complicates accounting. Skip.
2. **Rebasing share token** — share count changes per user, breaks composability with everything (Jupiter Swap, Kamino collateral, etc.). Solana protocols universally avoid this.

The exchange-rate model is what `TrustlessEscrowV3.sol` already does (ERC-4626 with `_decimalsOffset=6`); Kamino's `kvUSDC`, Solend's `cUSDC`, Jupiter Lend's `jlUSDC`, and MarginFi's bank shares all use the same conceptual mechanism. Mitigate first-depositor inflation by minting a tiny "dead" share to the program PDA at pool init (Solana equivalent of decimals-offset).

## Account / instruction layout

| Instruction | Signers | Key accounts | Notes |
|---|---|---|---|
| `init_pool(asset_mint, params)` | governance | Pool PDA (init), share_mint (init), usdc_vault_ata (init) | One-time |
| `deposit(amount)` | LP | LP usdc ATA, usdc_vault_ata, Pool PDA, LP share ATA, share_mint | Mints `amount * total_shares / total_assets` |
| `withdraw(shares)` | LP | reverse of deposit | **Constraint**: `usdc_vault_ata.amount >= preview_redeem(shares)`. If deployed capital exceeds idle, tx fails — this is the idle-only enforcement |
| `request_advance(receivable_id, amount)` | agent | agent USDC ATA, usdc_vault_ata, Pool PDA, Advance PDA (init), receivable_oracle program, oracle data accounts, credit_oracle program | CPIs into oracles, asserts caps, increments `deployed_amount`, transfers USDC out via PDA signer |
| `settle(advance_id, payment_proof)` | anyone | Advance PDA, usdc_vault_ata, agent ATA, protocol_treasury ATA, receivable_oracle | Verifies payment via oracle CPI (or accepts a Pyth/sig-verified proof account), runs waterfall, updates `total_assets` upward, decrements `deployed_amount`, marks settled |
| `liquidate(advance_id)` | anyone after expiry | Advance PDA, Pool PDA | Marks loss; subtracts from `total_assets` (LPs eat it pro-rata via share-price drop) |
| `propose_params / execute_params` | governance | Pool PDA, pending_params | 2-step timelock |
| `register_oracle(program_id, ratio_bps)` | governance | Pool PDA | Timelocked |

The agent passes the oracle program + its data accounts in the `request_advance` instruction's account list — Solana's "all accounts declared upfront" model means the client builds the full dependency graph. CredMesh's existing `buildAdvanceCalldata` server endpoint maps cleanly to a server-side `buildAdvanceInstruction` that returns base64 instructions ready to sign.

**Idle-only withdrawal accounting**: store `deployed_amount: u64` on the Pool PDA. `request_advance` does `deployed_amount += principal`, `settle/liquidate` does `deployed_amount -= principal`. `withdraw` checks `usdc_vault_ata.amount >= shares_to_underlying(shares)` — since deployed USDC has physically left the vault ATA, this enforces idle-only by construction without a separate check (matches the `TrustlessEscrowV3` semantics).

## Oracle composability pattern

**CPI to a trait-conforming oracle program** is the idiomatic Solana pattern when oracle logic is custom (reputation, registry receivables). For purely price-feed needs, you'd use Pyth/Switchboard pull oracles where the *caller passes the price account* and the program verifies it. CredMesh oracles aren't price feeds — they're stateful registries — so they should be deployed as separate programs:

```rust
// Each oracle program exposes a stable instruction discriminator
// e.g., 'verify_receivable' with a fixed account layout

// In credmesh_escrow::request_advance:
invoke(
    &Instruction {
        program_id: ctx.accounts.receivable_oracle.key(),
        accounts: oracle_account_metas,  // passed by client
        data: VerifyReceivable { agent, receivable_id }.try_to_vec()?,
    },
    &[/* AccountInfos */]
)?;
// Read the oracle's response from a PDA it wrote to,
// or have the oracle return data via return_data syscall (sol_set_return_data)
```

Use `set_return_data` / `get_return_data` (the Solana syscall pair, max 1024 bytes) for synchronous oracle reads — this is what Drift, Kamino, and others use for in-CPI value returns. For larger payloads, oracle writes to a known PDA the escrow then reads. `ReputationCreditOracle` translates to a tiny program that owns a `ReputationRegistry` PDA mapping `agent_pubkey -> ReputationData`; the escrow CPIs to read max-credit. Drift's multi-oracle enum (`OracleSource::Pyth | Switchboard | QuoteAsset | ...`) is a good model for the registry: store `Vec<(OracleProgramId, OracleKind, ratio_bps)>` and dispatch in code.

## Waterfall & fee accrual

**Waterfall**: keep the exact `treasury.ts` logic but in Rust. On `settle`, the agent (or settler) deposits the receivable proceeds into the program; the instruction splits via checked arithmetic in this order: principal → LP fee (85%) → protocol fee (15%) → late penalty → agent net. Use `MAX_WITHDRAW_AMOUNT`-style sentinels (Jupiter Lend pattern) for "settle full".

**Fee accrual to LPs**: exchange-rate increase, identical to `TrustlessEscrowV3`. When LP-portion of fee lands in `usdc_vault_ata`, do not mint new shares — `total_shares` stays fixed, so `share_price = total_assets / total_shares` rises. Total-assets is computed as `usdc_vault_ata.amount + deployed_amount - accrued_protocol_fees - unclaimed_remainders`. Protocol fees accrue separately and are pushed to `protocol_treasury` ATA on settle (or held in an escrow PDA, claimed via separate ix).

No Solana protocol does *exactly* this multi-tranche split for credit (Solend/Kamino/MarginFi only do principal+interest; Jupiter Lend uses a flat performance/management fee on the vault). Loopscale's order-book model is closest in spirit (per-loan repayment ledger) and confirms that per-advance `Advance` PDAs with their own ledger is the right shape.

## Trust / governance model

Pin three guarantees:

1. **No admin can approve/deny advances.** The `request_advance` instruction has no governance signer in its account list. Issuance is purely conditional on oracle CPI returns + cap checks. Even the program-upgrade authority cannot front-run this without a full upgrade — and upgrades should be timelocked (see #3).
2. **No pause function.** Don't write one. (Acknowledge that USDC mint-pause by Circle is the only stop button.)
3. **Timelocked governance.** Set program upgrade authority to a **Squads v4 multisig** with a built-in time-lock (Squads v4 supports per-vault time-locks natively). Pool.governance points to the same Squads vault. All param changes go through `propose_params` → wait `timelock_delay` → `execute_params`, exactly as the EVM contract does. Eventually transfer upgrade authority to `None` to make the program immutable (matches CredMesh's "minimized trust" framing).

This achieves what `TrustlessEscrowV3.sol` does on EVM: governance can tune knobs and add oracles slowly; governance cannot approve individual advances or halt the protocol.

## Token rails

Use **plain SPL Token** for USDC (mainnet `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v`, devnet `4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU`). Solana USDC has no transfer fee, no transfer hook, 6 decimals — same as EVM. The share token can also be plain SPL Token. CCTP/Gateway is only relevant for cross-chain flows (e.g., bridging from Base); not part of the local vault.

Devnet has a Circle USDC faucet (faucet.circle.com → "Solana Devnet"). Devnet liquidity is fine for E2E tests; for real LP behavior most teams move to mainnet-with-tiny-amounts because Solana mainnet tx costs are ~$0.0001 — the EVM "Sepolia first" instinct doesn't fully translate. Recommend **devnet for development, mainnet-beta for staging with hard caps** ($10–100 advances), skipping a public testnet entirely.

## Open questions

- **Oracle return-data size**: 1024-byte cap on `set_return_data` is fine for reputation/credit ints but tight if a receivable proof needs to carry signed payload + metadata. May need a "write to PDA, escrow reads PDA" pattern instead.
- **Per-advance NFT vs PDA**: Jupiter Lend issues an NFT per borrow position (transferable). CredMesh advances are not transferable (tied to agent identity), so plain PDAs are simpler — but should they be NFT-wrapped for explorer/portfolio compatibility? Open design call.
- **Trustless mode tx assembly**: Solana's "declare all accounts upfront" means an agent-built tx must include the right oracle-program + oracle-data accounts. If the oracle registry is dynamic, the client may need to fetch registry state right before signing. Address Lookup Tables (ALTs) help fit this in one tx but add a tiny stale-data risk.
- **Replay protection for `paymentTxHash`**: on Solana, "tx hash" is a 64-byte signature. Need to confirm whether `consumedPayments` should track tx signatures or a hash of (signature || receivable_id). Probably the latter, because Solana sigs aren't unique to a payment.
- **Squads v4 native timelock granularity**: confirmed it supports time-locks per multisig but unclear whether per-instruction-type delays are supported. May need a thin wrapper program if you want different delays for different param types (mirrors EVM's `pendingHardCapExecuteAfter` granularity).
- **First-depositor inflation attack on Solana**: ERC-4626's `_decimalsOffset=6` workaround doesn't have a direct SPL equivalent. Need to confirm the standard Solana mitigation (likely: program mints dead shares to itself at init).

## Key references

- Kamino Lending architecture (LendingMarket / Reserve / Obligation, cTokens) — https://www.mintlify.com/kamino-finance/klend/concepts/architecture
- Kamino kTokens (yield-bearing receipt mechanism) — https://kamino.com/docs/liquidity/ktokens
- Solend (Save) cTokens & user instructions — https://docs.save.finance/architecture/ctokens, https://docs.save.finance/architecture/user-instructions
- MarginFi v2 Bank/MarginfiAccount model — https://docs.marginfi.com/mfi-v2 / protocol-design
- Jupiter Lend (Fluid) — `helius:jupiter` skill `references/jupiter-lend.md`
- Loopscale Credit Order Book (per-loan PDA + repayment ledger) — https://docs.loopscale.com/protocol-concepts/loans-and-orders
- Drift multi-oracle enum + CPI dispatch — https://www.mintlify.com/drift-labs/protocol-v2/concepts/oracles
- Squads v4 timelocks + program upgrade authority — https://github.com/Squads-Protocol/v4, https://squads.so/blog/solana-multisig-program-upgrades-management
