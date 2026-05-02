# ORCHESTRATOR

You are running this file as the orchestrator Claude session. Your job: drive the four-track parallel plan from EPIC #9 by spawning four worker Claude sessions in tmux, monitoring them, and dispatching handoffs.

**Model**: one orchestrator Claude (you) + N workers (each its own `claude` process in a tmux window, each in its own git worktree). You drive everything via the Bash tool. The tmux-orchestrator skill explains the mechanics.

## Before you do anything

1. **Invoke the `tmux-orchestrator` skill via the Skill tool.** It documents the verified gotchas (separate Enter keypress, literal mode, multiline via `load-buffer`/`paste-buffer`, ANSI stripping, status-file handoffs). Do not skip this — every production tmux-claude setup gets one of these wrong without it.
2. **Read `EPIC #9`** via `gh issue view 9` so you understand the four tracks, dependencies, and handoff conditions.
3. **Read `WORKTREES.md`** for the per-track starter prompts and conflict surface map.

## Pre-flight checks

```bash
tmux has-session -t credmesh-orchestrate 2>/dev/null && \
  echo "session already exists; kill it first or attach" && exit
git worktree list                            # any track-A/B/C/D already checked out?
gh auth status                               # gh CLI authed
which claude                                 # claude CLI on PATH
```

If a previous run left worktrees, decide whether to keep them (continue) or remove them (`git worktree remove ../credmesh-solana-track-X`).

## Setup (one-shot)

```bash
SESSION="credmesh-orchestrate"
REPO_ROOT="$(pwd)"          # assume you're in credmesh-solana
PARENT="$(dirname "$REPO_ROOT")"

# Start from a clean main
git checkout main && git pull origin main

# Worktrees (skip if exists)
for trk in a:track-A-deploy b:track-B-reputation c:track-C-escrow d:track-D-tests; do
  name="${trk%%:*}"; branch="${trk##*:}"
  dir="$PARENT/credmesh-solana-track-$name"
  [ -d "$dir" ] || git worktree add "$dir" "$branch"
done

# Tmux session
tmux new-session -d -s "$SESSION" -n orchestrator -c "$REPO_ROOT"
```

## Spawn each worker

For each track in {a, b, c, d}:

```bash
NAME=a    # then b, c, d
DIR="$PARENT/credmesh-solana-track-$NAME"
PROMPT="$REPO_ROOT/scripts/orchestrate/track-$NAME-prompt.md"

tmux new-window -t "$SESSION" -n "track-$NAME" -c "$DIR"

# Launch claude (TWO separate send-keys calls — see skill)
tmux send-keys -t "$SESSION:track-$NAME" -l "claude --dangerously-skip-permissions"
sleep 0.5
tmux send-keys -t "$SESSION:track-$NAME" Enter

# Wait for Claude to be ready. Don't grep for a specific prompt char (it
# changes between Claude Code versions). Just sleep ~8s — empirically enough
# for boot in a clean shell.
sleep 8

# Inject starter prompt via load-buffer + paste-buffer (handles multiline)
tmux load-buffer -b "starter-$NAME" "$PROMPT"
tmux paste-buffer -p -d -b "starter-$NAME" -t "$SESSION:track-$NAME"
sleep 0.5
tmux send-keys -t "$SESSION:track-$NAME" Enter
```

**If the worker doesn't start typing within ~30s after Enter**, your `claude` binary may not be on PATH inside the new tmux window's shell. Fix: in the tmux launch step, prefix the command with the absolute path: `tmux send-keys -t … -l "$(which claude) --dangerously-skip-permissions"`.

## Verify all four are alive

```bash
for n in a b c d; do
  echo "=== track-$n ==="
  tmux capture-pane -t "$SESSION:track-$n" -p | sed 's/\x1b\[[0-9;]*[a-zA-Z]//g' | tail -10
done
```

Each should show Claude actively reading files or doing initial setup.

## Monitoring loop

Use the `loop` skill (`/loop 10m` to fire this monitoring routine every 10 minutes), OR use ScheduleWakeup with `delaySeconds: 600`. **Don't sit in a busy loop** — you'd burn context on no-op iterations.

Each tick:

```bash
# 1. Read status files
for n in a b c d; do
  f="/tmp/agent-track-$n.status"
  [ -f "$f" ] && echo "track-$n: $(cat "$f")"
done

# 2. Check liveness via capture-pane (last 5 lines, ANSI-stripped)
for n in a b c d; do
  out=$(tmux capture-pane -t "credmesh-orchestrate:track-$n" -p -S -20 \
        | sed 's/\x1b\[[0-9;]*[a-zA-Z]//g' | tail -5)
  echo "=== track-$n recent ==="
  echo "$out"
done

# 3. Check open PRs (workers should be opening them)
gh pr list --state open --json number,title,headRefName
```

After capturing, decide:

- **Did any track's status file flip to a known handoff signal?** Dispatch the next track:
  - Track A status `build_green` → unblock Track D (paste the unblock message into `credmesh-orchestrate:track-d`)
  - Track B status `reputation_typed_export_stable: true` → unblock Track C day 4 (paste into track-c)
- **Is any worker stuck?** Same `capture-pane` output for ≥3 consecutive ticks (≥30 min) means stuck. Send `C-c` then a recovery prompt.
- **Are PRs landing?** If yes, after merge tell the affected track to rebase.

## Handoff dispatch (paste the right message)

```bash
unblock_track_d() {
  msg="Track A reports anchor build is green. You are unblocked. Begin Day 1 of your plan."
  echo "$msg" | tmux load-buffer -b nudge-d -
  tmux paste-buffer -p -d -b nudge-d -t "credmesh-orchestrate:track-d"
  sleep 0.5
  tmux send-keys -t "credmesh-orchestrate:track-d" Enter
}

unblock_track_c_day4() {
  msg="Track B has merged emit_cpi (PR for #3). You can begin Day 4 (#4): typed cross-program reads via seeds::program."
  echo "$msg" | tmux load-buffer -b nudge-c -
  tmux paste-buffer -p -d -b nudge-c -t "credmesh-orchestrate:track-c"
  sleep 0.5
  tmux send-keys -t "credmesh-orchestrate:track-c" Enter
}
```

## Recovery (worker stuck)

```bash
recover() {
  local n=$1
  tmux send-keys -t "credmesh-orchestrate:track-$n" C-c
  sleep 1
  msg="You appeared stuck (no output for 30+ minutes). Read /tmp/agent-track-$n.status to recall where you were, summarize current state, then continue. If you can't proceed, write a status file with reason='blocked' and stop."
  echo "$msg" | tmux load-buffer -b recover-$n -
  tmux paste-buffer -p -d -b recover-$n -t "credmesh-orchestrate:track-$n"
  sleep 0.5
  tmux send-keys -t "credmesh-orchestrate:track-$n" Enter
}
```

## Reporting to the user

Each tick, after gathering status, append a one-line summary to `ORCHESTRATOR_LOG.md`:

```bash
{
  echo "## $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "- track-a: $(cat /tmp/agent-track-a.status 2>/dev/null || echo 'no status')"
  echo "- track-b: $(cat /tmp/agent-track-b.status 2>/dev/null || echo 'no status')"
  echo "- track-c: $(cat /tmp/agent-track-c.status 2>/dev/null || echo 'no status')"
  echo "- track-d: $(cat /tmp/agent-track-d.status 2>/dev/null || echo 'no status')"
  echo "- open PRs: $(gh pr list --state open --json number -q 'length')"
  echo ""
} >> ORCHESTRATOR_LOG.md
```

When the user asks "status", read this log + any new status files + recent capture-pane output, summarize in one paragraph.

## Tracking with TaskCreate

Use TaskCreate to model the four tracks at the high level (not the per-day work — that's each worker's job to track). Update task status when:
- A track's first status file appears → `in_progress`
- A track's PR for its final issue merges → `completed`
- A track is stuck >2 ticks → `blocked` (and recover)

## Termination

When the EPIC checklist is fully checked:

```bash
gh issue view 9                               # confirm checklist done
tmux kill-session -t credmesh-orchestrate
for n in a b c d; do
  git worktree remove "$PARENT/credmesh-solana-track-$n" || true
  git branch -d "track-$([[ $n == a ]] && echo A-deploy || \
                          [[ $n == b ]] && echo B-reputation || \
                          [[ $n == c ]] && echo C-escrow || \
                          echo D-tests)"
done
```

## What you do NOT do

- Don't write code yourself — workers do that
- Don't approve PRs yourself — flag them to the user, let them merge
- Don't enter worker tmux windows interactively — drive via send-keys
- Don't `git push` from main yourself — push-to-main is policy-blocked anyway

## When the user says "go"

Execute the Setup section, then the four Spawn-each-worker iterations, then start the monitoring loop. Report back to the user when all four workers are running.
