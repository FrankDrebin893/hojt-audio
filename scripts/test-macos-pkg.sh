#!/usr/bin/env bash
# Tests the macOS package's build script on any system: runs installer/macos/build-pkg.sh over stand-in
# bundles with stand-ins for pkgbuild and productbuild that record how they were called, and checks the
# calls (twelve component packages, the install locations, not relocatable), the distribution.xml
# (well formed with xmllint when installed, six plugins with VST3 and CLAP ticked, every package it names
# was built) and check-payload.sh on a good and on two bad payload listings. It does not build a real
# package: that needs a Mac, and nobody has installed the result on one yet.
#
#   scripts/test-macos-pkg.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

if command -v shellcheck > /dev/null; then
    shellcheck "$root"/installer/macos/*.sh "$root/scripts/test-macos-pkg.sh"
else
    echo "shellcheck is not installed: skipped (CI runs it)"
fi

names=$(sed -n 's/^name = "\(.*\)"$/\1/p' "$root/bundler.toml")
[ "$(echo "$names" | wc -l)" -eq 6 ] || fail "bundler.toml does not name six plugins"

# Stand-in bundles, as the macOS bundler makes them: a folder each
bundled="$tmp/bundled"
for name in $names; do
    mkdir -p "$bundled/$name.vst3/Contents/MacOS" "$bundled/$name.clap/Contents/MacOS"
    echo "x" > "$bundled/$name.vst3/Contents/MacOS/$name"
    echo "x" > "$bundled/$name.clap/Contents/MacOS/$name"
done

# Stand-ins for the Apple tools
shims="$tmp/shims"
mkdir "$shims"
cat > "$shims/pkgbuild" << 'EOF'
#!/usr/bin/env bash
echo "pkgbuild $*" >> "$SHIM_LOG"
if [ "$1" = --analyze ]; then
    printf '<plist><array><dict>\n<key>BundleIsRelocatable</key>\n<true/>\n</dict></array></plist>\n' > "$4"
    exit 0
fi
# the last argument is the package; the component plist must say not relocatable
while [ $# -gt 1 ]; do
    if [ "$1" = --component-plist ]; then
        grep -A1 BundleIsRelocatable "$2" | grep -q '<false/>' || { echo "relocatable" >&2; exit 1; }
    fi
    shift
done
touch "$1"
EOF
cat > "$shims/productbuild" << 'EOF'
#!/usr/bin/env bash
echo "productbuild $*" >> "$SHIM_LOG"
while [ $# -gt 1 ]; do
    case "$1" in
        --distribution) cp "$2" "$SHIM_OUT/distribution.xml" ;;
        --package-path) ls "$2" > "$SHIM_OUT/packages.txt" ;;
        --resources) ls "$2" > "$SHIM_OUT/resources.txt" ;;
    esac
    shift
done
touch "$1"
EOF
chmod +x "$shims"/*
mkdir "$tmp/out"

export SHIM_LOG="$tmp/log" SHIM_OUT="$tmp/out"
PATH="$shims:$PATH" HOJT_BUNDLED="$bundled" HOJT_DIST="$tmp/dist" bash "$root/installer/macos/build-pkg.sh" > /dev/null \
    || fail "build-pkg.sh failed"
[ -f "$tmp/dist/HojtDSP-macos.pkg" ] || fail "no HojtDSP-macos.pkg"

# Twelve component packages, each to its folder, with its own identifier
[ "$(grep -c -- '--install-location /Library/Audio/Plug-Ins/VST3 ' "$tmp/log")" -eq 6 ] || fail "not six VST3 packages to /Library/Audio/Plug-Ins/VST3"
[ "$(grep -c -- '--install-location /Library/Audio/Plug-Ins/CLAP ' "$tmp/log")" -eq 6 ] || fail "not six CLAP packages to /Library/Audio/Plug-Ins/CLAP"
for name in $names; do
    for format in vst3 clap; do
        grep -q -- "--identifier net.hojte.hojtaudio.$name.$format " "$tmp/log" || fail "no package for $name.$format"
        grep -q "^$name-$format.pkg\$" "$tmp/out/packages.txt" || fail "$name-$format.pkg was not built"
    done
done
grep -q 'license.txt' "$tmp/out/resources.txt" || fail "the choices page lacks its licence"
grep -q 'welcome.html' "$tmp/out/resources.txt" || fail "the choices page lacks its welcome"

# The choices page
dist_xml="$tmp/out/distribution.xml"
if command -v xmllint > /dev/null; then
    xmllint --noout "$dist_xml" || fail "distribution.xml is not well formed"
else
    echo "xmllint is not installed: distribution.xml not parsed (CI runs it)"
fi
[ "$(grep -c '<pkg-ref id=.*\.pkg</pkg-ref>' "$dist_xml")" -eq 12 ] || fail "distribution.xml does not point at twelve packages"
for name in $names; do
    for format in vst3 clap; do
        grep -q "<line choice=\"$name.$format\"/>" "$dist_xml" || fail "no line for $name.$format"
        grep -q "<choice id=\"$name.$format\" .*start_selected=\"true\"" "$dist_xml" || fail "$name.$format is not ticked by default"
        grep -q ">$name-$format.pkg</pkg-ref>" "$dist_xml" || fail "$name-$format.pkg is not named"
    done
    grep -q "<choice id=\"$name\" .*start_selected=\"true\"" "$dist_xml" || fail "$name is not ticked by default"
done
grep -q 'customize="always"' "$dist_xml" || fail "the choices page is not always shown"

# The payload check: good, a bundle missing, something extra
good="$tmp/good.txt"
{
    echo "."
    for name in $names; do
        echo "./$name.vst3"
        echo "./$name.vst3/Contents/MacOS/$name"
        echo "./$name.clap"
        echo "./$name.clap/Contents/MacOS/$name"
    done
} > "$good"
bash "$root/installer/macos/check-payload.sh" --list "$good" > /dev/null || fail "check-payload.sh refuses a good payload"
first=$(echo "$names" | head -1)
grep -v "$first.clap" "$good" > "$tmp/missing.txt"
if bash "$root/installer/macos/check-payload.sh" --list "$tmp/missing.txt" > /dev/null 2>&1; then fail "check-payload.sh accepts a payload without $first.clap"; fi
{ cat "$good"; echo "./Stray.vst3/Contents/x"; } > "$tmp/extra.txt"
if bash "$root/installer/macos/check-payload.sh" --list "$tmp/extra.txt" > /dev/null 2>&1; then fail "check-payload.sh accepts a payload with a stray bundle"; fi

# The same from a product archive of twelve components, as pkgutil unpacks one. A stand-in pkgutil lists
# only the first components with --payload-files, as the real one does on a product, so reading that
# would miss most bundles
command -v cpio > /dev/null || fail "cpio is not installed (the payload check needs it, as macOS has it)"
# make_product <dir> [skip]: <dir>/<Name>-<format>.pkg/Payload (gzip of cpio) for every component but skip
make_product() {
    local dir="$1" skip="${2:-}" name format comp
    for name in $names; do
        for format in vst3 clap; do
            comp="$name-$format"
            [ "$comp" = "$skip" ] && continue
            mkdir -p "$dir/$comp.pkg"
            (cd "$bundled" && find "$name.$format" | sed 's|^|./|' | cpio -o -H odc 2> /dev/null | gzip > "$dir/$comp.pkg/Payload")
        done
    done
    echo "<pkg-info/>" > "$dir/Distribution"
}
make_product "$tmp/product-good"
make_product "$tmp/product-missing" "$first-clap"
cat > "$shims/pkgutil" << 'EOF'
#!/usr/bin/env bash
case "$1" in
    --expand) cp -R "$SHIM_PRODUCT" "$3" ;;
    --payload-files) gzip -dc "$SHIM_PRODUCT"/*.pkg/Payload | cpio -it 2> /dev/null | head -4 ;;
esac
EOF
chmod +x "$shims/pkgutil"
SHIM_PRODUCT="$tmp/product-good" PATH="$shims:$PATH" bash "$root/installer/macos/check-payload.sh" "$tmp/x.pkg" > /dev/null \
    || fail "check-payload.sh refuses a good product archive"
if SHIM_PRODUCT="$tmp/product-missing" PATH="$shims:$PATH" bash "$root/installer/macos/check-payload.sh" "$tmp/x.pkg" > /dev/null 2>&1; then
    fail "check-payload.sh accepts a product archive without $first.clap"
fi

echo "macOS package script OK"
