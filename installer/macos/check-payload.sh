#!/usr/bin/env bash
# Checks what HojtDSP-macos.pkg installs against bundler.toml: every plugin's .vst3 and .clap is in the
# payload, and nothing else is at its top. On a Mac it lists the payload with `pkgutil --payload-files`
# (falling back to the expanded component payloads); a file of paths, one per line (./Name.vst3/...),
# can be given instead, which is how the test runs it anywhere.
#
#   installer/macos/check-payload.sh dist/HojtDSP-macos.pkg
#   installer/macos/check-payload.sh --list payload.txt
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
names=$(sed -n 's/^name = "\(.*\)"$/\1/p' "$root/bundler.toml")

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

if [ "${1:-}" = --list ]; then
    cp "${2:?usage: check-payload.sh --list <file>}" "$tmp/list"
else
    pkg="${1:?usage: check-payload.sh <pkg> | --list <file>}"
    pkgutil --payload-files "$pkg" > "$tmp/list" || true
    if ! grep -q 'vst3\|clap' "$tmp/list"; then
        # A product archive may list nothing: unpack it and read each component's payload
        pkgutil --expand "$pkg" "$tmp/expanded"
        : > "$tmp/list"
        for payload in "$tmp"/expanded/*.pkg/Payload; do
            gzip -dc "$payload" | cpio -it 2> /dev/null >> "$tmp/list" || true
        done
    fi
fi

# The first path component, "./" dropped: what sits at the install location
sed 's|^\./||; s|/.*$||' "$tmp/list" | grep -v '^\.\?$' | sort -u > "$tmp/top" || true
for name in $names; do
    echo "$name.vst3"
    echo "$name.clap"
done | sort -u > "$tmp/expected"

status=0
while read -r f; do
    echo "payload is missing $f" >&2
    status=1
done < <(comm -13 "$tmp/top" "$tmp/expected")
while read -r f; do
    echo "payload has $f, which bundler.toml does not name" >&2
    status=1
done < <(comm -23 "$tmp/top" "$tmp/expected")
# A VST3 is a folder with its binary inside
for name in $names; do
    grep -q "^\(\./\)\?$name.vst3/Contents/" "$tmp/list" || { echo "$name.vst3 has no Contents in the payload" >&2; status=1; }
done
[ "$status" -eq 0 ] && echo "payload matches bundler.toml: $(wc -l < "$tmp/expected") bundles"
exit "$status"
