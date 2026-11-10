#!/usr/bin/env bash
# Writes SHA256SUMS.txt beside the release assets in a folder (default dist): one line per file,
# in `sha256sum` format, so `sha256sum -c SHA256SUMS.txt` checks a download. It never lists itself.
#
#   scripts/checksums.sh [folder]
set -euo pipefail

dir="${1:-dist}"
cd "$dir"
rm -f SHA256SUMS.txt
files=()
for f in *; do
  if [[ -f "$f" ]]; then files+=("$f"); fi
done
((${#files[@]})) || { echo "no files in $dir" >&2; exit 1; }
printf '%s\0' "${files[@]}" | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS.txt
cat SHA256SUMS.txt
