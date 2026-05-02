You are Agent A on credmesh-solana's parallel work plan. Branch: track-A-deploy.

Read in this order: CLAUDE.md, then run `gh issue view 9` (the EPIC) and
`gh issue view 7` (your scope).

Scope: issue #7 ONLY. Three days of work:

Day 1: generate the three program keypairs (`solana-keygen new -o
target/deploy/credmesh_escrow-keypair.json`, etc), run `anchor keys sync`,
update `programs/credmesh-shared/src/lib.rs` `program_ids::ESCROW`/`REPUTATION`/
`RECEIVABLE_ORACLE`, update `Anchor.toml`, run `anchor build`, fix any
Anchor 0.30 syntax issues that surface.

Day 2: write `scripts/init_oracle.ts`, `scripts/init_pool.ts`,
`scripts/deploy.ts`. Use `@coral-xyz/anchor` 0.30 and the Codama-generated
client (run `anchor build && codama run` first to produce it).

Day 3: first devnet deploy, document any gotchas in DEPLOYMENT.md.

DO NOT touch source files in:
  - programs/credmesh-escrow/src/
  - programs/credmesh-reputation/src/
  - programs/credmesh-receivable-oracle/src/

You MAY modify their `declare_id!` lines and Cargo.toml (this is part of
`anchor keys sync`).

Open one PR per logical chunk via `gh pr create`.

When you finish day 1 (anchor build is green), write a status file:
  echo '{"status":"build_green","day":1}' > /tmp/agent-track-a.status

This unblocks Track D. Also `gh issue comment 9 --body "Track A: build is green, Track D unblocked"`.

Use TodoWrite to track your day-by-day progress. Begin now.
