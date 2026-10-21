#!/usr/bin/env bash
# Launches agy (implementer) non-interactively on one task, in a slot worktree and a dev/* branch.
# Usage: [AGY_SLOT=pro|plus|…] [REPO_DIR=<worktree>] run_agy.sh <tasks/...md> <dev/branch> [model] [extra-notes-file]
# REPO_DIR defaults to ../tbk-wt-<slot> (next to the orchestrator worktree tbk-wt-orch).
# Body wrapped in main(): bash parses the whole file first, so editing it never breaks a running launch.
main() {
set -u
SLOT=${AGY_SLOT:-pro}
REPO=${REPO_DIR:-$(cd "$(dirname "$0")/../.." && cd .. && pwd)/tbk-wt-$SLOT}
TASK="$1"; BRANCH="$2"; MODEL="${3:-gemini-3.8-flash-high}"; EXTRA="${4:-}"
C=$HOME/.cache/tbk-orchestration; LOGDIR=${SCRATCH:-$C}/logs; mkdir -p "$LOGDIR"
LOG="$LOGDIR/$(basename "$TASK" .md)-$SLOT-$(date +%H%M%S).log"
cd "$REPO" || exit 2
case "$BRANCH" in dev/*) ;; *) echo "branch must start with dev/"; exit 2;; esac
if git show-ref --verify --quiet "refs/heads/$BRANCH"; then git checkout -q "$BRANCH"; else git checkout -q -b "$BRANCH" develop-AGENTS; fi || exit 2
[ -f "$TASK" ] || { echo "task file not found on $BRANCH: $TASK"; exit 2; }
EXTRA_TXT=""; [ -n "$EXTRA" ] && EXTRA_TXT="$(cat "$EXTRA")"
PROMPT="You are the implementing agent of this repository. Your assigned task is: $TASK
You are on git branch '$BRANCH' (already checked out; work and commit only here).

Instructions:
1. Follow AGENTS.md strictly (it overrides everything). Read AGENTS.md, ALL of .ai-context/, tasks/README.md, your task file and every doc it references (always docs/protocol-spec.md) BEFORE writing code.
2. Implement the WHOLE task. Do not ask for confirmation: no human is available. On ambiguity choose the most conservative option that satisfies .ai-context and docs/protocol-spec.md and write it in the task Report. Never invent protocol bytes.
3. Run and make pass from the repo root (AGENTS.md section 8): cargo fmt --all --check; cargo clippy --workspace --all-targets --all-features -- -D warnings; cargo clippy -p transbank-pos --no-default-features -- -D warnings; cargo test --workspace --all-features; RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --all-features. Fix errors for real (no allow attributes, no ignored tests, no unwrap/expect in production code).
4. Commit on '$BRANCH' with Conventional Commits and 'Refs: TASK-<ID>'. At the end update the Estado row and the '## Reporte' section of your task file with REAL results, commit, and git push -u origin $BRANCH.
5. NEVER touch main, develop or develop-AGENTS. Never modify sibling repositories. Never open real serial ports. Never run cargo publish.
$EXTRA_TXT"
echo "=== $(date -u +%Y-%m-%dT%H:%M:%SZ) START task=$TASK branch=$BRANCH slot=$SLOT model=$MODEL" > "$LOG"
# Reuse the host-wide cargo lock wrappers of the owner's other repos when present.
W=$HOME/.cache/telemetry-orchestration
[ -x "$W/bin/cargo" ] && export PATH="$W/bin:$W/tools/bin:$PATH"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2} CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_TEST_DEBUG=line-tables-only
T=${AGY_TIMEOUT:-14400}
WL="/tmp/agy-$(basename "$PWD").lock"
if command -v flock >/dev/null 2>&1; then LOCK=(flock -n -E 75 "$WL"); else LOCK=(lockf -k -t 0 "$WL"); fi
"${LOCK[@]}" perl -e '$t=shift; alarm $t; exec @ARGV or die "exec: $!"' "$T" \
  agy -p "$PROMPT" --dangerously-skip-permissions --model "$MODEL" >> "$LOG" 2>&1
RC=$?
[ "$RC" -eq 75 ] && echo "BUSY: another agent is already running in $PWD" | tee -a "$LOG"
echo "=== $(date -u +%Y-%m-%dT%H:%M:%SZ) END rc=$RC" >> "$LOG"
echo "LOG=$LOG RC=$RC"
}
main "$@"
exit $?
