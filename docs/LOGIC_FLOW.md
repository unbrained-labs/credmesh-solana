# Logic Flow

Sequence diagrams for the canonical handlers. Pair with `docs/ARCHITECTURE.md` for the static structure.

## LP deposit (`escrow::deposit`)

```mermaid
sequenceDiagram
    autonumber
    participant LP
    participant ESC as escrow program
    participant Pool as Pool PDA
    participant Vault as USDC vault (token acct)
    participant Mint as Share mint
    LP->>ESC: deposit(amount)
    ESC->>Pool: read total_assets, total_shares
    ESC->>ESC: shares = preview_deposit(amount, total_assets, total_shares)<br/>(virtual-shares offset, OZ ERC-4626 pattern)
    LP->>Vault: token::transfer(amount) → vault
    ESC->>Pool: total_assets += amount<br/>total_shares += shares
    ESC->>Mint: token::mint_to(LP, shares)
    Note over LP,Mint: First-depositor inflation defense via virtual offsets:<br/>1-atom donation costs ≥ 10⁶ × any extractable profit
```

## Withdraw (`escrow::withdraw`)

```mermaid
sequenceDiagram
    autonumber
    participant LP
    participant ESC as escrow program
    participant Pool as Pool PDA
    participant Vault as USDC vault
    participant Mint as Share mint
    LP->>ESC: withdraw(shares)
    ESC->>Pool: read total_assets, total_shares, deployed_amount
    ESC->>ESC: assets_out = preview_redeem(shares, total_assets, total_shares)
    ESC->>ESC: require!(assets_out ≤ idle = total_assets - deployed_amount)
    ESC->>Mint: token::burn(LP, shares)
    Vault->>LP: token::transfer(assets_out) (PDA-signed)
    ESC->>Pool: total_assets -= assets_out<br/>total_shares -= shares
    Note over LP,Vault: Idle-only enforcement: cannot withdraw locked-as-deployed funds
```

## Request advance — Worker source path (`escrow::request_advance` source_kind=0)

```mermaid
sequenceDiagram
    autonumber
    participant Agent
    participant ESC as escrow program
    participant Rep as reputation program
    participant Recv as receivable-oracle program
    participant Pool as Pool PDA
    participant Adv as Advance PDA (init)
    participant Cons as ConsumedPayment PDA (init)
    participant Vault as USDC vault
    participant ATA as agent USDC ATA
    Agent->>ESC: request_advance(receivable_id, amount, source_kind=0)
    ESC->>Rep: typed Account<AgentReputation> read<br/>seeds=[REPUTATION, agent_asset], seeds::program=reputation::ID<br/>(Anchor 0.30 4-step verify auto)
    ESC->>Recv: typed Account<Receivable> read (Some)<br/>seeds=[RECEIVABLE, [0u8], agent, receivable_id]<br/>seeds::program=receivable_oracle::ID<br/>(Worker namespace, post-#32 fix)
    ESC->>ESC: staleness ≤ MAX_STALENESS_SLOTS
    ESC->>ESC: amount ≤ min(receivable*pct/10000, abs_cap, credit_from_score_ema)<br/>fee = compute_fee_amount(curve, util, duration, default_count)
    ESC->>ESC: init Advance PDA<br/>seeds=[ADVANCE, pool, agent, receivable_id]
    ESC->>ESC: init ConsumedPayment PDA<br/>seeds=[CONSUMED, pool, agent, receivable_id]<br/>(replay protection — PERMANENT)
    Vault->>ATA: token::transfer(amount) (PDA-signed)
    ESC->>Pool: deployed_amount += amount<br/>require!(deployed ≤ total_assets)
    ESC-->>Agent: emit AdvanceIssued
    Note over Agent,ATA: emit! is the LAST step (project convention)
```

## Request advance — Ed25519/X402 path (`escrow::request_advance` source_kind=1|2)

```mermaid
sequenceDiagram
    autonumber
    participant Cli as Caller (offchain)
    participant Agent
    participant ESC as escrow program
    participant Rep as reputation program
    participant Sysvar as Instructions sysvar
    participant Pool as Pool PDA
    participant Adv as Advance PDA (init)
    participant Cons as ConsumedPayment PDA (init)
    participant Vault as USDC vault
    Cli->>Cli: build SignedReceivable (96B layout)<br/>sign with allowed_signer key
    Cli->>ESC: TX = [ed25519_verify_ix, request_advance_ix]
    ESC->>Sysvar: load_instruction_at_checked(prev_ix)
    ESC->>ESC: verify_prev_ed25519(): asymmetric.re/Relay-class<br/>fix preserved (binds verify ix to current ix)
    ESC->>ESC: SignedReceivable::decode() → msg_recv_id, msg_agent,<br/>amount, expires_at, nonce
    ESC->>ESC: require!(msg_agent == agent_asset.key())<br/>(cross-agent replay defense)
    ESC->>Rep: typed Account<AgentReputation> read
    ESC->>ESC: receivable_pda = None (intentional;<br/>ed25519 path doesn't read PDA)
    ESC->>ESC: cap checks + fee calc (same as Worker)
    ESC->>ESC: init Advance + ConsumedPayment
    Vault->>Agent: token::transfer(amount) (PDA-signed)
    ESC->>Pool: deployed_amount += amount
    ESC-->>Agent: emit AdvanceIssued
```

## Settlement waterfall (`escrow::claim_and_settle`)

```mermaid
sequenceDiagram
    autonumber
    participant Cranker as Cranker (== advance.agent in v1, AUDIT P0-3)
    participant ESC as escrow program
    participant Adv as Advance PDA
    participant Cons as ConsumedPayment PDA
    participant Pool as Pool PDA
    participant Memo as Memo v2 ix
    participant PayerAta as payer USDC ATA
    participant Vault as USDC vault
    participant ProtoAta as protocol treasury ATA
    participant AgentAta as agent USDC ATA
    Cranker->>ESC: TX = [memo_ix(nonce), claim_and_settle(payment_amount)]
    ESC->>Memo: require_memo_nonce(MAX_IX_SCAN=64)<br/>finds memo with expected nonce<br/>(post-#32 capped scan)
    ESC->>Adv: read principal, fee_owed, expires_at
    ESC->>Cons: assert exists with same agent + receivable_id
    ESC->>ESC: late_days = max(0, now - expires_at) / 86400 (clamped MAX_LATE_DAYS)<br/>late_penalty = late_days × adv.late_penalty_per_day<br/>total_fee = fee_owed + late_penalty
    ESC->>ESC: protocol_cut = total_fee × 1500 / 10000 (u128)<br/>lp_fee = total_fee - protocol_cut<br/>lp_cut = principal + lp_fee<br/>agent_net = payment_amount - protocol_cut - lp_cut
    ESC->>ESC: require!(protocol_cut + lp_cut + agent_net == payment_amount)
    PayerAta->>Vault: transfer(lp_cut)
    PayerAta->>ProtoAta: transfer(protocol_cut)
    alt agent_net > 0
        PayerAta->>AgentAta: transfer(agent_net)
    end
    ESC->>Pool: deployed_amount -= principal<br/>total_assets += lp_fee<br/>accrued_protocol_fees += protocol_cut
    Note over ESC,Pool: total_assets and accrued_protocol_fees<br/>are SEPARATE LEDGERS by design.<br/>skim_protocol_fees touches the latter only.
    ESC->>Adv: state = Settled, close=agent (rent → agent)
    ESC->>Cons: NOT closed — permanent (AUDIT P0-5)
    ESC-->>Cranker: emit AdvanceSettled
```

## Liquidation (`escrow::liquidate`)

```mermaid
sequenceDiagram
    autonumber
    participant Cranker
    participant ESC as escrow program
    participant Adv as Advance PDA
    participant Cons as ConsumedPayment PDA
    participant Pool as Pool PDA
    Cranker->>ESC: liquidate()
    ESC->>Adv: read expires_at, principal
    ESC->>ESC: require!(now ≥ expires_at + LIQUIDATION_GRACE_SECONDS)
    ESC->>Cons: assert consumed.agent == advance.agent (AUDIT P0-1)
    ESC->>Pool: deployed_amount -= principal<br/>total_assets -= principal (LPs eat the loss)
    ESC->>Adv: state = Liquidated (NOT closed — AUDIT AM-7 keeps audit trail)
    ESC->>Cons: NOT closed — permanent
    ESC-->>Cranker: emit AdvanceLiquidated
    Note over Cranker,Pool: Cannot liquidate before deadline + grace window.<br/>Share price drops; total_shares unchanged.
```

## Reputation update (`reputation::give_feedback`)

```mermaid
sequenceDiagram
    autonumber
    participant Cranker
    participant REP as reputation program
    participant Cfg as OracleConfig (cross-program read)
    participant Agent as AgentReputation PDA
    Cranker->>REP: give_feedback(score, feedback_uri, prev_digest_hash)
    REP->>Cfg: typed Account<OracleConfig> read<br/>seeds=[ORACLE_CONFIG], seeds::program=oracle::ID
    alt attestor == oracle_config.reputation_writer_authority
        REP->>REP: gate passes — score updates allowed
        REP->>Agent: digest = sha256(prev_digest || feedback_uri || score)<br/>(8004-shape rolling)
        REP->>Agent: score_ema = EMA(score, alpha=64/1024)
        REP->>Agent: feedback_count += 1
        REP->>Agent: if score < default_threshold: default_count += 1
        REP->>Agent: last_event_slot = current_slot
    else attestor != writer
        REP->>Agent: digest update + count bump only<br/>(permissionless audit trail)<br/>NO score change
    end
    REP-->>Cranker: emit_cpi! NewFeedback<br/>(via inner-instruction, defends 10KB log truncation)
    Note over Cranker,Agent: emit_cpi! survives noisy-log adversary tx<br/>(AUDIT defended via PR #11)
```

## Receivable recording — Worker path (`oracle::worker_update_receivable`)

```mermaid
sequenceDiagram
    autonumber
    participant Worker as Worker authority (off-chain oracle)
    participant REC as receivable-oracle program
    participant Cfg as OracleConfig
    participant Recv as Receivable PDA (init_if_needed)
    Worker->>REC: worker_update_receivable(source_id, amount, expires_at)
    REC->>Cfg: constraint: config.worker_authority == worker.key()
    REC->>REC: lazy period reset if past worker_period_seconds
    REC->>REC: require!(amount ≤ worker_max_per_tx)<br/>require!(period_used + amount ≤ worker_max_per_period)
    REC->>Recv: init or update PDA<br/>seeds=[RECEIVABLE, [0u8], agent, source_id]<br/>(post-#32 namespaced by source_kind)
    REC->>Recv: source_kind=0, source_signer=None,<br/>amount, expires_at, last_updated_slot
    REC->>Cfg: worker_period_used += amount
    REC-->>Worker: emit ReceivableUpdated
```

## Receivable recording — Ed25519 path (`oracle::ed25519_record_receivable`)

```mermaid
sequenceDiagram
    autonumber
    participant Cli as Caller (offchain)
    participant Payer
    participant REC as receivable-oracle program
    participant Sysvar as Instructions sysvar
    participant AS as AllowedSigner PDA
    participant Recv as Receivable PDA (init_if_needed)
    Cli->>Cli: build 96B SignedReceivable<br/>sign with allowed_signer key
    Payer->>REC: TX = [ed25519_verify_ix, ed25519_record_receivable]
    REC->>Sysvar: verify_prev_ed25519() returns (verified_pubkey, signed_msg)
    REC->>REC: 4-layer binding:<br/>(a) ed25519 native verify<br/>(b) verified_pubkey == ix-arg signer_pubkey<br/>(c) AllowedSigner.signer == signer_pubkey<br/>(d) msg fields == ix args (recv_id, agent, amount, expires_at)
    REC->>AS: read kind, caps
    REC->>REC: lazy period reset; per-receivable + per-period cap checks
    REC->>Recv: init or update PDA<br/>seeds=[RECEIVABLE, [allowed_signer.kind], agent, source_id]<br/>(post-#32 namespaced by signer kind 1 or 2)
    REC->>Recv: source_kind=signer_acc.kind, source_signer=Some(signer_pubkey),<br/>amount, expires_at
    REC->>AS: period_used += amount
    REC-->>Payer: emit ReceivableUpdated
```

---

## Critical invariants enforced across the diagrams

| Invariant | Where | Defends |
|---|---|---|
| `deployed_amount ≤ total_assets` | request_advance post-state, withdraw pre-state | Pool can't deploy more than LPs deposited |
| `cranker == advance.agent` | claim_and_settle, liquidate constraint | AUDIT P0-3/P0-4 — v1 cranker permissioning |
| `consumed.agent == advance.agent` | claim_and_settle, liquidate | AUDIT P0-1 — cross-agent settle defense |
| ConsumedPayment PERMANENT | no close handler anywhere | AUDIT P0-5 — close-then-reinit replay defense |
| Pool has no `paused` field | state.rs | AUDIT P0-6 — issuance never gated |
| `init` (not `init_if_needed`) for replay PDAs | request_advance Advance + ConsumedPayment | AUDIT P0-5 |
| `total_assets` and `accrued_protocol_fees` = SEPARATE LEDGERS | claim_and_settle waterfall | LP price not inflated by skimmable fees |
| Memo nonce scan capped at 64 ix | require_memo_nonce | Post-#32 — DoS defense for v1.5 permissionless cranking |
| FeeCurve invariants validated at init+propose | init_pool, propose_params | Post-#32 — governance footgun guard |
| Receivable PDA namespaced by source_kind | seeds in oracle handlers | Post-#32 — cross-path overwrite defense |
