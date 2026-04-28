# credmesh-solana

Research and design for porting [CredMesh](https://github.com/unbrainedORG/credmesh) — a programmable credit protocol for autonomous agents — from EVM (Base / Sepolia) to Solana.

## Status

**Pre-implementation research.** No Solana code yet. The EVM protocol is live at https://credmesh.xyz.

## Layout

```
research/
  01-vault-escrow.md       On-chain vault, escrow, advance issuance, oracle composability, governance
  02-identity-reputation.md ERC-8004 → Solana Agent Registry; permissionless reputation w/ rolling digest
  03-offchain-infra.md     [pending] Auth, tx submission, multi-cluster config, dashboard wallet adapter
  04-payments-oracles.md   x402 equivalent, fiat ramps, receivable oracles, atomic repayment
  SYNTHESIS.md             End-to-end architecture, EVM→Solana mapping, phased plan
  REVIEW.md                Critical pass on SYNTHESIS — supersedes it where they conflict
```

**Read order**: REVIEW.md first → SYNTHESIS.md → numbered reports for detail.

## Research method

Four parallel research agents, each scoped to one layer of the system, briefed with the relevant EVM contracts/code paths from the credmesh monorepo. Each agent invoked the appropriate Helius skill (build / svm / phantom / jupiter / dflow / okx) plus Exa web search before writing.

## Next steps once research is complete

1. Single-document synthesis (SYNTHESIS.md) reconciling the four reports.
2. Anchor program scaffolding decision (single program vs split escrow/identity/reputation/oracles).
3. Devnet deployment plan.
4. Dashboard fork strategy (new package in credmesh monorepo, or sibling project).
