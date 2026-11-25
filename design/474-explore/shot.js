const { chromium } = require('playwright');
const path = require('path');
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1100, height: 584 }, deviceScaleFactor: 1 });
  let bad = 0;
  for (const name of process.argv.slice(2)) {
    await p.goto('file://' + path.resolve(name + '.html'));
    await p.evaluate(() => document.fonts.ready);
    await p.waitForTimeout(300);
    const H = await p.evaluate(() => +document.body.dataset.h);
    await p.setViewportSize({ width: 1100, height: H });
    const issues = await p.evaluate((H) => {
      const els = [...document.querySelectorAll('.c')].filter(e => !e.closest('[data-ctx]'));
      const R = e => e.getBoundingClientRect();
      const out = [];
      const label = e => (e.textContent || '').trim().slice(0, 18) || e.className;
      for (const e of els) { const r = R(e); if (r.left < -1 || r.top < -1 || r.right > 1101 || r.bottom > H + 1) out.push(['outside window', label(e)]); }
      for (let i = 0; i < els.length; i++) for (let j = i + 1; j < els.length; j++) {
        const a = els[i], c = els[j];
        if (a.contains(c) || c.contains(a)) continue;
        const ra = R(a), rb = R(c);
        const w = Math.min(ra.right, rb.right) - Math.max(ra.left, rb.left);
        const h = Math.min(ra.bottom, rb.bottom) - Math.max(ra.top, rb.top);
        if (w > 1 && h > 1) out.push(['overlap', label(a), label(c), Math.round(w) + 'x' + Math.round(h), Math.round(Math.max(ra.left, rb.left)), Math.round(Math.max(ra.top, rb.top))]);
      }
      for (const e of document.querySelectorAll('.cap,.tape,.silkt,.tag')) if (e.scrollWidth > e.clientWidth + 1) out.push(['text wider than box', e.textContent.trim().slice(0, 20), e.scrollWidth, e.clientWidth]);
      return out;
    }, H);
    await p.screenshot({ path: name + '.png' });
    console.log(name, issues.length ? 'ISSUES ' + issues.length : 'clean');
    issues.slice(0, 40).forEach(i => console.log('  ', JSON.stringify(i)));
    bad += issues.length;
  }
  await b.close();
  process.exit(bad ? 1 : 0);
})();
