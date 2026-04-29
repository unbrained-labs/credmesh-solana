You are Agent D on credmesh-solana's parallel work plan. Branch: track-D-tests.

Read in this order: CLAUDE.md, AUDIT.md (the attack fixtures map to AUDIT
P0/P1/P2 findings), then `gh issue view 9` (EPIC), `gh issue view 6`.

Scope: issue #6 ONLY. Test files in `tests/bankrun/` exclusively.

START GATE: read `/tmp/agent-track-a.status`. If the file does not exist or
`status` is not `build_green`, you are blocked. Read `tests/bankrun/setup.ts`
and the existing test stubs to understand the harness, but do NOT write
new code yet. Poll the status file every 60 seconds.

Once Track A is green, four days of work:

Day 1: happy paths. Fill in `tests/bankrun/escrow/init_pool.test.ts` and
`tests/bankrun/escrow/deposit_withdraw.test.ts`. These don't depend on Tracks
B/C work-in-flight. Open PR per file.

Day 2: `request_advance` happy path (worker source). Property tests:
waterfall sum invariant, share-price monotonicity, first-depositor inflation
defense (1-atom donation costs ≥ 10⁶× attacker profit). Open PR.

Day 3: attack fixtures. `consumed_close_reinit.test.ts` lands AFTER Track C
#8 lands (poll /tmp/agent-track-c.status for issue 8 complete).
`ata_substitution.test.ts`, `sysvar_spoofing.test.ts`,
`cross_agent_replay.test.ts` can land any time. Open PR per fixture.

Day 4: ed25519 path tests + claim_and_settle waterfall + liquidate. These
assert on layouts Track C #2 finalizes — coordinate timing.

If a test depends on a layout/handler Track C is mid-flight on, write the
test against current main + leave a TODO + open a DRAFT PR. Don't block on
Track C's merges; iterate.

DO NOT touch any source under `programs/`. Tests-only.

Write status as PRs merge:
  echo '{"status":"day_X_complete","files":["..."]}' > /tmp/agent-track-d.status

Pull main DAILY to absorb merged work from other tracks. Use TodoWrite. Begin now.
