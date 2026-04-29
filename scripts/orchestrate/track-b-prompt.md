You are Agent B on credmesh-solana's parallel work plan. Branch: track-B-reputation.

Read in this order: CLAUDE.md, then `gh issue view 9` (the EPIC),
`gh issue view 3` (emit_cpi), `gh issue view 2` (InitSpace).

Scope: issue #3 + the reputation portion of #2 ONLY. Three days:

Day 1: #3. In `programs/credmesh-reputation/src/lib.rs`, add `#[event_cpi]`
to the `GiveFeedback` accounts struct, swap `emit!(NewFeedback)` to
`emit_cpi!`. Other events stay `emit!`. Open PR.

Day 2: #2 reputation portion. Add `#[derive(InitSpace)]` to `AgentReputation`
in `programs/credmesh-reputation/src/state.rs`. Drop the manual `SIZE` const.
Update `init_reputation` accounts struct: `space = 8 + AgentReputation::INIT_SPACE`.
Open PR.

Day 3: Bankrun fixture proving `emit_cpi!` event survives a noisy-log
adversary tx. Test in `tests/bankrun/reputation/`. Open PR.

DO NOT touch source files in:
  - programs/credmesh-escrow/src/
  - programs/credmesh-receivable-oracle/src/

When PR for #3 merges, write status:
  echo '{"status":"emit_cpi_merged","reputation_typed_export_stable":true}' > /tmp/agent-track-b.status

This unblocks Track C's day 4 (#4).

Use TodoWrite to track progress. Pull main into your branch DAILY to absorb
merged work from other tracks. Begin now.
