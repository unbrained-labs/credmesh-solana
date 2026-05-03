# PROGRESS

Snapshot of the repo state at major milestones. Most-recent first.

## 2026-05-03 — post-EPIC #9 + audit-driven fixes + first devnet deploy

- All 7 EPIC #9 child issues' work landed on main (#2/#3/#4/#6/#7/#8 closed; #5 intentionally a permanent DRAFT PR for v1.5+).
- 5-pass audit (4 Claude code-reviewers + 1 Kimi K2 via forge) — see `AUDIT.md § Post-EPIC #9 audit pass`.
- 3 audit-driven MED fixes shipped in PR #32 (Receivable PDA namespace, memo loop cap, FeeCurve validation).
- 1 compile-discovered fix in PR #34 (`anchor-lang` `event-cpi` feature flag).
- 2 of 3 programs deployed to devnet, both with verifiable-build SHA256 matches:
  - `credmesh-reputation` → `JDBeDr9WFhepcz4C2JeGSsMN2KLW4C1aQdNLS2jvc79G`
  - `credmesh-receivable-oracle` → `ALVf6iyB6P5RFizRtxorJ3pAcc4731VziAn67sW6brvk`
  - `credmesh-escrow` → keypair reserved at `DLy82HRrSnSVZfQTxze8CEZwequnGyBcJNvYZX1L9yuF`, deploy pending wallet top-up
- Architecture + logic-flow Mermaid diagrams in `docs/`.
- DEPLOYMENT.md `§ Devnet deploy log` records actual program IDs + slots + ProgramData addresses.

Outstanding for mainnet (per V1_ACCEPTANCE.md mainnet-readiness gates):
1. Deploy `credmesh-escrow` once wallet has ≥3.5 SOL.
2. Init flows (`init_oracle`, `init_pool`) with real governance + worker-authority + treasury USDC ATA.
3. Squads CPI verification on `propose_params` (currently a `Signer<'info>` constraint).
4. IDL extraction fix (issue #15) — unblocks TS-client typed-tx + harness-mode bankrun activation.
5. External audit firm engagement.
6. Rotate program-deploy keypairs + transfer upgrade authority to Squads vault.

## Earlier session (2026-04-23..29)

EVM-to-Solana port. Pre-implementation scaffolding, three independent design reviews (DESIGN.md + AUDIT.md), all 6 P0 fund-loss findings fixed before EPIC #9 began.
