#!/usr/bin/env bash
# Moves the pin of our nih-plug fork to another commit: the old rev (read from the nih_plug line of the
# root Cargo.toml) becomes <rev> in Cargo.toml and Cargo.lock, and its first eight characters in the
# commit CLAUDE.md names. Prints each file with the lines changed. It does not fetch or build: run
# `cargo check --workspace --locked` after it. See "A change in the nih-plug fork", plugins/README.md.
#
#   scripts/pin-nih-plug.sh <40-hex rev>
#
# Exits 1 when the rev is not 40 lower-case hex characters or the old rev is not found; the same rev is
# "already pinned" (exit 0). NIH_PLUG_PIN_ROOT names another directory to work in (the test uses it).
set -euo pipefail

root="${NIH_PLUG_PIN_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
new="${1:-}"
[[ "$new" =~ ^[0-9a-f]{40}$ ]] || {
    echo "usage: scripts/pin-nih-plug.sh <40-hex commit of FrankDrebin893/nih-plug>" >&2
    exit 1
}

old="$(sed -nE 's|^nih_plug = \{ git = "https://github.com/FrankDrebin893/nih-plug.git", rev = "([0-9a-f]{40})".*|\1|p' "$root/Cargo.toml")"
[[ -n "$old" ]] || {
    echo "pin-nih-plug: no nih_plug line with a 40-hex rev in $root/Cargo.toml" >&2
    exit 1
}
if [[ "$old" == "$new" ]]; then
    echo "already pinned: $old"
    exit 0
fi

# The whole 40 characters, so no other hash matches by a prefix; CLAUDE.md names the short form too
changed=0
for f in Cargo.toml Cargo.lock CLAUDE.md; do
    [[ -f "$root/$f" ]] || continue
    if [[ "$f" == CLAUDE.md ]]; then
        n="$(grep -c "${old:0:8}" "$root/$f" || true)"
    else
        n="$(grep -c "$old" "$root/$f" || true)"
    fi
    ((n > 0)) || continue
    tmp="$(mktemp)"
    sed -e "s/$old/$new/g" -e "s/${old:0:8}/${new:0:8}/g" "$root/$f" > "$tmp"
    cat "$tmp" > "$root/$f"
    rm -f "$tmp"
    echo "$f: $n line(s) changed"
    changed=$((changed + n))
done
((changed > 0)) || {
    echo "pin-nih-plug: $old was not found in any file" >&2
    exit 1
}
echo "pinned $old -> $new; now run: cargo check --workspace --locked"
