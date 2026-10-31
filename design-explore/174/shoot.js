const { chromium } = require('playwright');
(async () => {
  const dir = process.argv[2];
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 2640, height: 1180 } });
  for (const d of ['today', 'A', 'B', 'C']) {
    await p.goto('file://' + dir + '/explore.html?d=' + d);
    await p.evaluate(() => document.fonts.ready);
    await p.waitForTimeout(300);
    await p.screenshot({ path: dir + '/' + d + '.png', fullPage: true });
  }
  await b.close();
})();
