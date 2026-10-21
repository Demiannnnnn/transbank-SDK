#!/usr/bin/env bash
# Validates the current branch of REPO_DIR against develop-AGENTS. Compact output for the orchestrator.
# Usage: REPO_DIR=<worktree> [TAILN=30] validate.sh
main() {
set -u
W=$HOME/.cache/telemetry-orchestration
[ -x "$W/bin/cargo" ] && export PATH="$W/bin:$W/tools/bin:$PATH"
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-2} CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_TEST_DEBUG=line-tables-only
cd "${REPO_DIR:?REPO_DIR required}" || exit 2
B=$(git branch --show-current); echo "branch=$B"
git log --oneline develop-AGENTS..HEAD | head -20
echo "-- uncommitted:"; git status --short | head -10
echo "-- protected files touched:"; git diff --name-only develop-AGENTS...HEAD | grep -E "^(AGENTS\.md|CLAUDE\.md|GEMINI\.md|NOTICE|LICENSE-|\.ai-context/|docs/|tasks/README\.md|tools/orchestration/)" || echo "(none)"
echo "-- task files touched:"; git diff --name-only develop-AGENTS...HEAD | grep -E "^tasks/" || echo "(none)"
echo "-- new dependencies (check architecture.md §3):"; git diff develop-AGENTS...HEAD -- '*Cargo.toml' | grep -E '^\+[a-z0-9_-]+ *=' | grep -vE '^\+(name|version|edition|description|readme|license|repository|authors|rust-version|keywords|categories|include|workspace|path|default|serial|serde|tracing|tokio|members|resolver|documentation|homepage|required-features|doc|test|bench|harness)' || echo "(none)"
echo "-- cheats:"; git diff develop-AGENTS...HEAD -- . ':!Cargo.lock' ':!*.md' | grep -nE "^\+" | grep -E "(#!?\[allow\(|#!?\[expect\(|#\[ignore|unsafe|\.unwrap\(\)|\.expect\(|panic!\(|todo!\(|unimplemented!\(|thread::sleep|f32|f64|println!|eprintln!)" | head -40 || true
echo "-- stat:"; git diff --stat develop-AGENTS...HEAD | tail -1
run(){ local n=$1; shift; if out=$("$@" 2>&1); then echo "$n: OK"; else echo "$n: FAIL"; echo "$out" | grep -vE "^\s*$" | tail -"${TAILN:-30}"; fi; }
H=(); command -v heavy >/dev/null 2>&1 && H=(heavy)
[ -f Cargo.toml ] || { echo "no Cargo.toml yet (docs-only branch)"; return 0; }
run fmt cargo fmt --all --check
run clippy "${H[@]}" cargo clippy --workspace --all-targets --all-features -- -D warnings
run clippy-no-default "${H[@]}" cargo clippy -p transbank-pos --no-default-features -- -D warnings
run test "${H[@]}" cargo test --workspace --all-features
echo "-- rust test counts:"; "${H[@]}" cargo test --workspace --all-features 2>&1 | grep -E "^test result" | awk '{s+=$4} END {print s" passed"}'
run doc env RUSTDOCFLAGS="-D warnings" "${H[@]}" cargo doc --workspace --no-deps --all-features
}
main "$@"
exit $?
