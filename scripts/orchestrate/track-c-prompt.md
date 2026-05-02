You are Agent C on credmesh-solana's parallel work plan. Branch: track-C-escrow.

Read in this order: CLAUDE.md, AUDIT.md (every "AUDIT" comment in source
references it), DESIGN.md, research/HANDLER_PATTERNS.md, then
`gh issue view 9` (EPIC), then in order: `gh issue view 8`, `gh issue view 2`,
`gh issue view 4`, `gh issue view 5`.

Scope: issues #8, #2 (escrow portion), #4, #5 — IN THAT ORDER, SERIALLY.
Each touches overlapping lines in escrow/lib.rs and escrow/state.rs.
Do NOT parallelize within this track. Five days:

Day 1: #8. Add `agent.key()` to `ConsumedPayment` PDA seeds. Update every
seed-derivation site (request_advance, claim_and_settle, liquidate). Add
Bankrun fixture in `tests/bankrun/attacks/` proving cross-agent receivable_id
reuse no longer collides. Update DESIGN.md + AUDIT.md. Open PR.

Day 2-3: #2 escrow portion. `#[derive(InitSpace)]` on `Pool`, `Advance`,
`ConsumedPayment`, `PendingParams`, `FeeCurve`. Replace every `space = SIZE`
with `8 + INIT_SPACE`. **Verify the previously hidden `PendingParams::SIZE`
undercount is corrected** — DESIGN says it omits `execute_after`'s 8 bytes.
Open PR.

Day 4: #4. Wait for Track B's emit_cpi PR to merge first (check
/tmp/agent-track-b.status for `reputation_typed_export_stable: true`).
Then migrate `agent_reputation_pda` and `receivable_pda` from
`UncheckedAccount` + manual `read_cross_program_account` to typed
`Account<'info, T>` with `seeds::program`. Drop the manual reads at those
sites. Open PR.

Day 5: #5 SPIKE ONLY. Feature-flag a `token_interface` migration; do NOT
merge to main yet. Land behind a `token-2022` Cargo feature for future
activation. Open draft PR.

DO NOT touch source files in:
  - programs/credmesh-reputation/src/ (except day 4, only the import sites in escrow that consume reputation types — NOT reputation source itself)

End of each day, write status:
  echo '{"status":"day_X_complete","issue":"N"}' > /tmp/agent-track-c.status

Use TodoWrite to track progress. Pull main DAILY. Begin now with #8.
