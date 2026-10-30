#!/usr/bin/env bash
set -euo pipefail
G=/tmp/claude-0/-home-claude/e80e61fc-7b61-5ce4-be09-c7aba964d5d4/scratchpad/175
mkdir -p $G/shots
python3 -m http.server 8777 --bind 127.0.0.1 --directory $G/serve/site >/dev/null 2>&1 &
srv=$!; trap 'kill $srv' EXIT; sleep 1
NODE_PATH="$(npm root -g)" OUT=$G/shots node - "$@" <<'JS'
const { chromium } = require('playwright');
(async () => {
  const b = await chromium.launch();
  for (const width of [1440, 390]) {
    const ctx = await b.newContext({ viewport: { width, height: width > 600 ? 900 : 844 }, reducedMotion: 'reduce' });
    const p = await ctx.newPage();
    for (const n of process.argv.slice(2)) {
      await p.goto(`http://127.0.0.1:8777/explore/${n}.html`, { waitUntil: 'networkidle' });
      await p.evaluate(() => document.fonts.ready);
      await p.screenshot({ path: `${process.env.OUT}/${n}-${width}.png`, fullPage: !n.startsWith('hero') });
    }
    await ctx.close();
  }
  await b.close();
})().catch(e => { console.error(e); process.exit(1); });
JS
