#!/usr/bin/env bash
# Tests scripts/pin-nih-plug.sh on a temporary copy of the files it edits: a rev is replaced everywhere (and
# egui-baseview's own rev is left alone), a bad rev is refused, a second run is "already pinned", and nothing
# is written when the old rev is not found.
#
#   scripts/test-pin-nih-plug.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

if command -v shellcheck > /dev/null; then
    shellcheck "$root/scripts/pin-nih-plug.sh" "$root/scripts/test-pin-nih-plug.sh"
else
    echo "shellcheck is not installed: skipped (CI runs it)"
fi

old=f7b1196b266961c70528c7bb3737b9069c13b238
new=0123456789abcdef0123456789abcdef01234567
other=ec70c3fe6b2f070dcacbc22924431edbe24bd1c0
pin() { NIH_PLUG_PIN_ROOT="$tmp" bash "$root/scripts/pin-nih-plug.sh" "$@"; }

cat > "$tmp/Cargo.toml" << TOML
nih_plug = { git = "https://github.com/FrankDrebin893/nih-plug.git", rev = "$old" }
nih_plug_egui = { git = "https://github.com/FrankDrebin893/nih-plug.git", rev = "$old" }
egui-baseview = { git = "https://github.com/BillyDM/egui-baseview.git", rev = "$other" }
TOML
cat > "$tmp/Cargo.lock" << LOCK
source = "git+https://github.com/FrankDrebin893/nih-plug.git?rev=$old#$old"
source = "git+https://github.com/BillyDM/egui-baseview.git?rev=$other#$other"
LOCK
echo "pinned at commit \`${old:0:8}\`" > "$tmp/CLAUDE.md"

pin "$new" > /dev/null || fail "a good rev was refused"
! grep -rq "$old" "$tmp" || fail "the old rev is still somewhere"
[ "$(grep -c "$new" "$tmp/Cargo.toml")" -eq 2 ] || fail "Cargo.toml does not carry the new rev twice"
[ "$(grep -c "$new" "$tmp/Cargo.lock")" -eq 1 ] || fail "Cargo.lock does not carry the new rev"
grep -q "$other" "$tmp/Cargo.toml" || fail "egui-baseview's rev was touched"
grep -q "${new:0:8}" "$tmp/CLAUDE.md" || fail "CLAUDE.md's short commit was not replaced"

pin "$new" | grep -q "already pinned" || fail "a second run is not 'already pinned'"

if pin abc123 2> /dev/null; then fail "a short rev was accepted"; fi
if pin "${new^^}" 2> /dev/null; then fail "an upper-case rev was accepted"; fi

# No old rev to read (no nih_plug line in Cargo.toml) while Cargo.lock and CLAUDE.md do carry one: refused,
# and neither file is written
echo "[workspace]" > "$tmp/Cargo.toml"
printf 'source = "git+https://github.com/FrankDrebin893/nih-plug.git?rev=%s#%s"\n' "$old" "$old" > "$tmp/Cargo.lock"
echo "pinned at commit \`${old:0:8}\`" > "$tmp/CLAUDE.md"
cp "$tmp/Cargo.lock" "$tmp/before.lock"
cp "$tmp/CLAUDE.md" "$tmp/before.md"
if pin "$new" 2> /dev/null; then fail "a Cargo.toml without the pin was not refused"; fi
cmp -s "$tmp/Cargo.lock" "$tmp/before.lock" || fail "Cargo.lock was written although the old rev could not be read"
cmp -s "$tmp/CLAUDE.md" "$tmp/before.md" || fail "CLAUDE.md was written although the old rev could not be read"
echo "== pin-nih-plug: all checks pass"
