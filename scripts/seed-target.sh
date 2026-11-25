#!/usr/bin/env bash
# A pre-warmed build cache for a cloud session (issue 466): the first `scripts/check.sh` in a fresh worktree compiles
# egui, wgpu, naga and image from scratch (about 30 minutes). One machine gets one SEED, a target/ built once in the
# main checkout, which no builder uses (builders use worktrees); each builder's worktree gets a COPY of it. No session
# names the seed as its CARGO_TARGET_DIR: that would bring back the build-lock fights of a shared directory.
#
#   scripts/seed-target.sh start         starts the seed build detached and returns at once (the SessionStart hook calls it).
#                                        Does nothing while one runs (lock file target/.seed.lock holds its pid), when the
#                                        done marker target/.seed-done matches the Rust and Cargo.lock, when the disk is
#                                        under the cold floor (check.sh space), or without CLAUDE_CODE_REMOTE=true.
#                                        The build is the two check.sh runs, `cargo test --workspace --no-run` and
#                                        `cargo test -p amp -p bass --profile optimised --no-run`; log: target/seed.log
#   scripts/seed-target.sh wait          blocks until no seed build runs; exits 1 (and prints the log's last line) when
#                                        there is no finished seed for this Rust and Cargo.lock
#   scripts/seed-target.sh copy <dir>    copies the seed into <dir>/target (cp -a, no incremental, no logs), unless that
#                                        target/ is warm already (has debug/deps), there is no finished seed or the disk
#                                        is under the cold floor (8 GB); copied through a temporary directory. A plain
#                                        copy, not hardlinks: cargo rewrites files in place and would write into the seed.
#                                        Never writes into the seed
#
# SEED_CARGO_JOBS=<n> limits the seed build's jobs (CARGO_BUILD_JOBS), so it leaves cores to the first builder.
set -euo pipefail

self="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
here="$(cd "$(dirname "$self")/.." && pwd)"
# The seed lives in the main checkout: the first worktree git lists
main="$(git -C "$here" worktree list --porcelain 2> /dev/null | sed -n 's/^worktree //p' | head -1)"
[[ -n "$main" ]] || main="$here"
seed="$main/target"
lock="$seed/.seed.lock"
marker="$seed/.seed-done"
log="$seed/seed.log"

# What the seed was built from: the Rust and the lock file. The marker also names the commit, for a person.
fingerprint() {
    echo "rustc: $(rustc -V 2> /dev/null || echo unknown)"
    echo "cargo-lock: $(sha256sum "$main/Cargo.lock" 2> /dev/null | cut -d' ' -f1)"
}

seed_is_current() {
    [[ -f "$marker" ]] || return 1
    local line
    while IFS= read -r line; do
        grep -Fxq -- "$line" "$marker" || return 1
    done < <(fingerprint)
}

running() {
    local pid
    [[ -f "$lock" ]] || return 1
    pid="$(cat "$lock" 2> /dev/null || true)"
    [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2> /dev/null
}

need_remote() {
    [[ "${CLAUDE_CODE_REMOTE:-}" == true ]] || {
        echo "seed-target: not a cloud session (CLAUDE_CODE_REMOTE is not true), nothing to do"
        exit 0
    }
}

# The disk check for a seed build and for a copy: both write a cold build's size (2 to 7 GB), whatever the main
# checkout's target/ looks like, so the cold floor (8 GB) applies; CHECK_MIN_FREE_GB overrides it
enough_space() {
    (cd "$main" && CHECK_MIN_FREE_GB="${CHECK_MIN_FREE_GB:-8}" bash scripts/check.sh space > /dev/null 2>&1)
}

start() {
    need_remote
    if running; then
        echo "seed-target: a seed build is running already (pid $(cat "$lock"))"
        return 0
    fi
    if seed_is_current; then
        echo "seed-target: the seed is current ($(head -1 "$marker" | tr -d '\n')), skipped"
        return 0
    fi
    if ! enough_space; then
        echo "seed-target: not enough disk space for a seed build, skipped"
        return 0
    fi
    mkdir -p "$seed"
    rm -f "$marker"
    : > "$log"
    # setsid: the build outlives the hook that starts it
    setsid nohup bash "$self" build > "$log" 2>&1 < /dev/null &
    local pid=$! waited=0
    # the build writes its own lock (see build), so a build that ends at once cannot leave a stale one behind
    while [[ ! -f "$lock" ]] && kill -0 "$pid" 2> /dev/null && ((waited++ < 100)); do sleep 0.1; done
    echo "seed-target: seed build started (pid $pid), log $log"
}

# Internal: the build itself, run detached by `start`. Removes the lock when it ends; writes the marker on success.
build() {
    echo $$ > "$lock"
    trap 'rm -f "$lock"' EXIT
    cd "$main"
    unset CARGO_TARGET_DIR
    [[ -z "${SEED_CARGO_JOBS:-}" ]] || export CARGO_BUILD_JOBS="$SEED_CARGO_JOBS"
    echo "seed build: $(date -u +%FT%TZ) cargo test --workspace --no-run --locked"
    cargo test --workspace --no-run --locked
    echo "seed build: $(date -u +%FT%TZ) cargo test -p amp -p bass --profile optimised --no-run --locked"
    cargo test -p amp -p bass --profile optimised --no-run --locked
    {
        fingerprint
        echo "commit: $(git rev-parse HEAD 2> /dev/null || echo unknown)"
    } > "$marker"
    echo "seed build: $(date -u +%FT%TZ) done"
}

wait_for() {
    need_remote
    while running; do sleep 5; done
    if seed_is_current; then
        echo "seed-target: the seed is done"
        return 0
    fi
    echo "seed-target: no finished seed; last line of the log: $(tail -n 1 "$log" 2> /dev/null || echo none)" >&2
    exit 1
}

copy() {
    local dir="${1:-}" dest entry name sub
    [[ -d "$dir" ]] || {
        echo "usage: scripts/seed-target.sh copy <worktree>" >&2
        exit 64
    }
    dest="$(cd "$dir" && pwd)/target"
    if [[ "$dest" == "$seed" ]]; then
        echo "seed-target: <dir> is the seed's own checkout, nothing to copy" >&2
        exit 64
    fi
    if [[ -d "$dest/debug/deps" ]]; then
        echo "seed-target: $dest is warm already, left alone"
        return 0
    fi
    # a seed of an older Cargo.lock still helps (cargo reuses what matches), but only a finished one: a build
    # that runs holds partial files
    if running || [[ ! -f "$marker" ]]; then
        echo "seed-target: no finished seed, $dest stays cold"
        return 0
    fi
    if ! enough_space; then
        echo "seed-target: not enough disk space for a copy (a copy is a cold build's size), $dest stays cold"
        return 0
    fi
    # Copy into a temporary directory and move the parts in, so an interrupted copy leaves nothing that reads as warm
    tmp="$dest.seed-copy" # not local: the EXIT trap reads it
    rm -rf "$tmp"
    trap 'rm -rf "$tmp"' EXIT
    mkdir -p "$tmp"
    for entry in "$seed"/* "$seed"/.[!.]*; do
        [[ -e "$entry" ]] || continue
        name="$(basename "$entry")"
        case "$name" in
        check-logs | seed.log | .seed.lock | .seed-done) continue ;;
        esac
        if [[ -d "$entry" && ( "$name" == debug || "$name" == optimised ) ]]; then
            mkdir -p "$tmp/$name"
            for sub in "$entry"/* "$entry"/.[!.]*; do
                [[ -e "$sub" ]] || continue
                [[ "$(basename "$sub")" == incremental ]] && continue
                cp -a --reflink=auto "$sub" "$tmp/$name/"
            done
        else
            cp -a --reflink=auto "$entry" "$tmp/"
        fi
    done
    mkdir -p "$dest"
    for entry in "$tmp"/* "$tmp"/.[!.]*; do
        [[ -e "$entry" ]] || continue
        rm -rf "${dest:?}/$(basename "$entry")"
        mv "$entry" "$dest/"
    done
    echo "seed-target: copied the seed into $dest"
}

case "${1:-}" in
start) start ;;
build) build ;;
wait) wait_for ;;
copy) copy "${2:-}" ;;
*)
    echo "usage: scripts/seed-target.sh start|wait|copy <worktree>" >&2
    exit 64
    ;;
esac
