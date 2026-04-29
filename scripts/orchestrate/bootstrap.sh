#!/usr/bin/env bash
# Bootstrap the four-track parallel work plan from EPIC #9 in tmux.
#
# Spawns one tmux session with five windows: orchestrator (you, attached) +
# four worker windows, each cd'd into its own git worktree, each running
# `claude --dangerously-skip-permissions` with a track-specific starter prompt.
#
# Reference skill: ~/.claude/skills/tmux-orchestrator/SKILL.md
# Prompts: scripts/orchestrate/track-{a,b,c,d}-prompt.md (edit before running)
#
# Usage:
#   ./scripts/orchestrate/bootstrap.sh           # set up worktrees + tmux + workers
#   tmux attach -t credmesh-orchestrate          # attach (workers already running)
#
# To tear down:
#   tmux kill-session -t credmesh-orchestrate
#   git worktree remove ../credmesh-solana-track-a (etc.)

set -euo pipefail

SESSION="credmesh-orchestrate"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PROMPTS_DIR="$REPO_ROOT/scripts/orchestrate"
PARENT_DIR="$(dirname "$REPO_ROOT")"

require() {
  command -v "$1" >/dev/null 2>&1 || { echo "missing dependency: $1" >&2; exit 1; }
}
require tmux
require git
require claude

# Refuse to run if session already exists — the user should kill it first.
if tmux has-session -t "$SESSION" 2>/dev/null; then
  echo "session $SESSION already exists. tmux kill-session -t $SESSION to start over." >&2
  exit 1
fi

# 1. Worktrees — idempotent (skip if already present)
ensure_worktree() {
  local name=$1
  local branch=$2
  local dir="$PARENT_DIR/credmesh-solana-track-$name"
  if [ -d "$dir" ]; then
    echo "worktree exists: $dir"
    return
  fi
  ( cd "$REPO_ROOT" && git worktree add "$dir" "$branch" )
}

( cd "$REPO_ROOT" && git fetch origin main && git checkout main && git pull origin main )

ensure_worktree a track-A-deploy
ensure_worktree b track-B-reputation
ensure_worktree c track-C-escrow
ensure_worktree d track-D-tests

# 2. Tmux session
tmux new-session -d -s "$SESSION" -n orchestrator -c "$REPO_ROOT"

# 3. Spawn each worker
spawn_worker() {
  local name=$1
  local worktree="$PARENT_DIR/credmesh-solana-track-$name"
  local prompt_file="$PROMPTS_DIR/track-$name-prompt.md"

  if [ ! -f "$prompt_file" ]; then
    echo "missing prompt: $prompt_file" >&2
    exit 1
  fi

  tmux new-window -t "$SESSION" -n "track-$name" -c "$worktree"

  # Launch claude
  tmux send-keys -t "$SESSION:track-$name" -l "claude --dangerously-skip-permissions"
  sleep 0.5
  tmux send-keys -t "$SESSION:track-$name" Enter

  # Wait for the prompt to appear (Claude Code shows "│ >" when ready for input)
  local i
  for i in $(seq 1 60); do
    sleep 1
    if tmux capture-pane -t "$SESSION:track-$name" -p | grep -q "│ >" 2>/dev/null; then
      break
    fi
  done

  # Inject starter prompt via load-buffer + paste-buffer (handles multiline cleanly)
  tmux load-buffer -b "starter-$name" "$prompt_file"
  tmux paste-buffer -p -d -b "starter-$name" -t "$SESSION:track-$name"
  sleep 0.5
  tmux send-keys -t "$SESSION:track-$name" Enter

  echo "spawned: track-$name → $worktree"
}

spawn_worker a
spawn_worker b
spawn_worker c
spawn_worker d

# 4. Final orchestrator pane
tmux send-keys -t "$SESSION:orchestrator" -l "echo 'Workers running. Attach with: tmux attach -t $SESSION'"
sleep 0.2
tmux send-keys -t "$SESSION:orchestrator" Enter

cat <<EOF

All four workers spawned in tmux session: $SESSION

Attach with:
  tmux attach -t $SESSION

Switch between windows: Ctrl-b 0..4  (orchestrator=0, track-a..d=1..4)
Detach without killing: Ctrl-b d
Kill everything:        tmux kill-session -t $SESSION

Status files (workers should write here):
  /tmp/agent-track-a.status
  /tmp/agent-track-b.status
  /tmp/agent-track-c.status
  /tmp/agent-track-d.status
EOF
