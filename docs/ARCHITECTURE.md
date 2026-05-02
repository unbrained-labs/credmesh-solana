# Architecture

CredMesh-Solana is a programmable credit protocol for autonomous agents on Solana. Three deployable programs + one shared library crate.

## High-level

```mermaid
graph TB
    subgraph "credmesh deployable programs"
        ESC[credmesh-escrow<br/>USDC pool, advances, waterfall, liquidate]
        REP[credmesh-reputation<br/>8004-shape rolling reputation digest]
        REC[credmesh-receivable-oracle<br/>worker + ed25519 payer-signed receivables]
    end

    subgraph "library crate"
        SHA[crates/credmesh-shared<br/>seeds, program_ids, cross-program helpers,<br/>ix introspection, ed25519 message layout]
    end

    subgraph "external Solana programs (consumed, not deployed)"
        SPL[SPL Token<br/>USDC vault + share mint]
        ED[ed25519 native program<br/>signed-receivable verify]
        MEMO[Memo v2<br/>replay-nonce binding]
        SQDS[Squads v4<br/>agent vaults + governance<br/>v1 deferred to v1.5]
        MPL[MPL Agent Registry +<br/>Agent Tools + Core<br/>agent identity, executive profile]
    end

    ESC -.uses.-> SHA
    REP -.uses.-> SHA
    REC -.uses.-> SHA

    ESC -- "typed Account&lt;AgentReputation&gt;<br/>seeds::program" --> REP
    ESC -- "typed Account&lt;Receivable&gt;<br/>seeds::program (Worker path)" --> REC
    REP -- "typed Account&lt;OracleConfig&gt;<br/>seeds::program" --> REC

    ESC --> SPL
    REC --> ED
    ESC --> MEMO

    ESC -.future.-> SQDS
    ESC -.future.-> MPL
```

## Per-program PDAs

```mermaid
graph LR
    subgraph "credmesh-escrow PDAs"
        Pool["Pool<br/>seeds: [POOL, asset_mint]<br/>fields: total_assets, total_shares,<br/>deployed_amount, accrued_protocol_fees,<br/>fee_curve, governance, treasury_ata"]
        Advance["Advance<br/>seeds: [ADVANCE, pool, agent, receivable_id]<br/>fields: principal, fee_owed,<br/>late_penalty_per_day, expires_at, state<br/>close=agent at settlement"]
        Consumed["ConsumedPayment<br/>seeds: [CONSUMED, pool, agent, receivable_id]<br/>PERMANENT — never closed (AUDIT P0-5)<br/>replay-protection ledger"]
    end

    subgraph "credmesh-reputation PDAs"
        Rep["AgentReputation<br/>seeds: [REPUTATION, agent_asset]<br/>fields: feedback_count,<br/>feedback_digest (32B rolling),<br/>score_ema (u128), default_count,<br/>last_event_slot"]
    end

    subgraph "credmesh-receivable-oracle PDAs"
        Cfg["OracleConfig<br/>seeds: [ORACLE_CONFIG]<br/>fields: governance, worker_authority,<br/>caps, reputation_writer_authority"]
        Recv["Receivable<br/>seeds: [RECEIVABLE, source_kind, agent, source_id]<br/>fields: amount, expires_at, source_kind,<br/>source_signer, last_updated_slot<br/>NAMESPACED by source_kind (PR #32)"]
        AS["AllowedSigner<br/>seeds: [ALLOWED_SIGNER, signer_pubkey]<br/>fields: kind (1=exchange, 2=x402),<br/>per-receivable + per-period caps"]
    end

    Pool -- one --> Advance
    Pool -- one --> Consumed
    Advance -. matches .- Consumed
```

## Three-key topology (DESIGN §10)

Three off-chain authorities that MUST never share keys:

```mermaid
graph LR
    FP[Fee-payer wallet<br/>pays SOL for tx fees<br/>not a signing authority]
    OW[Oracle worker authority<br/>signs worker_update_receivable<br/>caps in OracleConfig]
    RW[Reputation writer authority<br/>signs gated give_feedback writes<br/>controlled by oracle governance]

    FP -.never same as.- OW
    OW -.never same as.- RW
    FP -.never same as.- RW
```

If any two collapse to the same key, a single compromise yields cross-protocol takeover. Rotation flow lives in `DEPLOYMENT.md`.

## Workspace layout

```
programs/
  credmesh-escrow/                  Pool vault + share mint + advance issuance + waterfall settlement + liquidate
  credmesh-reputation/              AgentReputation 8004-shape rolling-digest reputation + writer-gated EMA updates
  credmesh-receivable-oracle/       Worker-attested + ed25519 payer-signed receivables + allowed-signer registry
crates/
  credmesh-shared/                  LIBRARY ONLY (not deployed). Seed constants, program-ID consts, cross-program
                                    helpers (read_cross_program_account 4-step verify), ix introspection
                                    (verify_prev_ed25519, require_memo_nonce), ed25519_message layout
ts/server/                          Hono backend (SIWS auth, tx-builder, webhook ingress)
tests/bankrun/                      anchor-bankrun: pure-math + scaffolded harness suites by program/attack class
scripts/                            deploy.ts, init_oracle.ts, init_pool.ts + lib helpers
target/deploy/                      Committed devnet program keypairs (escrow, reputation, receivable_oracle)
```

## Cross-program data flow at a glance

| Caller | Reads | Writes | Purpose |
|---|---|---|---|
| escrow.request_advance | reputation.AgentReputation, oracle.Receivable (Worker), or instructions sysvar (ed25519/x402) | escrow.Pool, escrow.Advance, escrow.ConsumedPayment | issue an advance |
| escrow.claim_and_settle | escrow.Advance, escrow.ConsumedPayment | escrow.Pool | settle waterfall |
| escrow.liquidate | escrow.Advance, escrow.ConsumedPayment | escrow.Pool | post-deadline writeoff |
| reputation.give_feedback | oracle.OracleConfig (writer-gating + cap) | reputation.AgentReputation | append feedback + update EMA |
| oracle.worker_update_receivable | oracle.OracleConfig (worker auth + caps) | oracle.Receivable (Worker namespace) | record off-chain attested receivable |
| oracle.ed25519_record_receivable | oracle.AllowedSigner, instructions sysvar | oracle.Receivable (Exchange/X402 namespace) | record payer-signed receivable |

See `docs/LOGIC_FLOW.md` for the per-handler sequence diagrams.
