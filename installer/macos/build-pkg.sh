#!/usr/bin/env bash
# Builds dist/HojtDSP-macos.pkg, the macOS installer, from the bundles in target/bundled: one component
# package per plugin and format (pkgbuild; VST3 installs to /Library/Audio/Plug-Ins/VST3, CLAP to
# /Library/Audio/Plug-Ins/CLAP), joined by productbuild with the choices page
# (make-distribution.sh). Unsigned. Needs macOS (pkgbuild, productbuild). Run after scripts/package.sh,
# which empties dist. UNTESTED on a Mac: nobody has installed it yet.
#
#   installer/macos/build-pkg.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
bundled="${HOJT_BUNDLED:-$root/target/bundled}"
dist="${HOJT_DIST:-$root/dist}"
here="$root/installer/macos"
id_prefix=net.hojte.hojtaudio

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$root/Cargo.toml" | head -1)
[ -n "$version" ] || { echo "no version in Cargo.toml" >&2; exit 1; }
names=$(sed -n 's/^name = "\(.*\)"$/\1/p' "$root/bundler.toml")

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/pkgs" "$work/resources" "$dist"

for name in $names; do
    for format in vst3 clap; do
        case "$format" in
            vst3) location=/Library/Audio/Plug-Ins/VST3 ;;
            clap) location=/Library/Audio/Plug-Ins/CLAP ;;
        esac
        [ -e "$bundled/$name.$format" ] || { echo "missing $bundled/$name.$format" >&2; exit 1; }
        stage="$work/root/$name-$format"
        mkdir -p "$stage"
        cp -R "$bundled/$name.$format" "$stage/"

        # Not relocatable: Installer would otherwise update a copy of the bundle found anywhere else
        # on the disk (a ~/Library one, say) instead of putting it where the choice says
        plist="$work/$name-$format.plist"
        pkgbuild --analyze --root "$stage" "$plist"
        sed '/BundleIsRelocatable/{n;s/<true\/>/<false\/>/;}' "$plist" > "$plist.new"
        mv "$plist.new" "$plist"

        pkgbuild --root "$stage" \
            --component-plist "$plist" \
            --identifier "$id_prefix.$name.$format" \
            --version "$version" \
            --install-location "$location" \
            --ownership recommended \
            "$work/pkgs/$name-$format.pkg"
    done
done

cp "$here"/resources/* "$work/resources/"
cp "$root/LICENSE" "$work/resources/license.txt"
bash "$here/make-distribution.sh" "$version" > "$work/distribution.xml"

productbuild --distribution "$work/distribution.xml" \
    --resources "$work/resources" \
    --package-path "$work/pkgs" \
    --version "$version" \
    "$dist/HojtDSP-macos.pkg"
echo "built $dist/HojtDSP-macos.pkg ($version)"
