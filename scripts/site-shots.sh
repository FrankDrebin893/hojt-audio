#!/usr/bin/env bash
# Screenshots of every page of the site (site/), so an agent can see what it built: each page at
# desktop (1440) and phone (390) width, full length, into target/shots/<page>-<width>.png.
# The page is scrolled to the end first, since the images below the first screen load lazily
# and a capture without the scroll shows them blank. Used by /design and /page.
#
#   scripts/site-shots.sh              every page
#   scripts/site-shots.sh dsp piano    only these (a folder under site/, "home" for the root)
#
# Needs Node with Playwright and its Chromium (in a cloud session both are there; elsewhere
# `npm i -g playwright && npx playwright install chromium`).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/target/shots"
mkdir -p "$out"

if [ $# -eq 0 ]; then
    pages=(home)
    while read -r f; do
        pages+=("${f%/index.html}")
    done < <(cd "$root/site" && find . -mindepth 2 -name index.html | sed 's|^\./||' | sort)
else
    pages=("$@")
fi

port=8765
python3 -m http.server "$port" --bind 127.0.0.1 --directory "$root/site" > /dev/null 2>&1 &
server=$!
trap 'kill $server 2> /dev/null' EXIT
for _ in $(seq 50); do
    curl -s -o /dev/null "http://127.0.0.1:$port/" && break
    sleep 0.1
done

NODE_PATH="${NODE_PATH:-$(npm root -g 2> /dev/null || true)}" \
OUT="$out" PORT="$port" node - "${pages[@]}" <<'JS'
const { chromium } = require('playwright');
(async () => {
  const pages = process.argv.slice(2);
  const browser = await chromium.launch();
  for (const width of [1440, 390]) {
    const context = await browser.newContext({
      viewport: { width, height: width > 600 ? 900 : 844 },
      deviceScaleFactor: 1,
      reducedMotion: 'reduce',
    });
    const page = await context.newPage();
    for (const name of pages) {
      const path = name === 'home' ? '' : `${name}/`;
      await page.goto(`http://127.0.0.1:${process.env.PORT}/${path}`, { waitUntil: 'networkidle' });
      // Scroll to the end and back, so every lazy image has loaded
      await page.evaluate(async () => {
        for (let y = 0; y < document.body.scrollHeight; y += 600) {
          window.scrollTo(0, y);
          await new Promise((r) => setTimeout(r, 60));
        }
        window.scrollTo(0, 0);
      });
      await page.waitForLoadState('networkidle');
      await page.evaluate(() => document.fonts.ready);
      const file = `${process.env.OUT}/${name.replace(/\//g, '-')}-${width}.png`;
      await page.screenshot({ path: file, fullPage: true });
      console.log(file);
    }
    await context.close();
  }
  await browser.close();
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
JS
