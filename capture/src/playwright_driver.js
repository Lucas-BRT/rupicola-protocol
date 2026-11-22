const pw = require(process.env.PW_DIR);
const engine = process.env.PW_ENGINE || 'chromium';
const width = Number(process.env.VP_W);
const height = Number(process.env.VP_H);
const actions = JSON.parse(process.env.ACTIONS_JSON || '[]');
const recordDir = process.env.VIDEO_DIR || '';

(async () => {
  const launcher = pw[engine] || pw.chromium;
  const browser = await launcher.launch();
  const ctxOpts = { viewport: { width, height } };
  if (recordDir) ctxOpts.recordVideo = { dir: recordDir, size: { width, height } };
  const ctx = await browser.newContext(ctxOpts);
  const page = await ctx.newPage();
  await page.goto(process.env.TARGET_URL, { waitUntil: 'networkidle' });

  for (const act of actions) {
    const sel = (id) => (id.startsWith('[') || id.startsWith('.') || id.startsWith('#')) ? id : '#' + id;
    if (act.action === 'type-text') await page.fill(sel(act.target_id), act.text);
    else if (act.action === 'click') await page.click(sel(act.target_id));
    else if (act.action === 'hover') await page.hover(sel(act.target_id));
    else if (act.action === 'focus') await page.focus(sel(act.target_id));
    else if (act.action === 'press-key') await page.keyboard.press(act.key);
    else if (act.action === 'wait-ms') await page.waitForTimeout(act.ms);
  }

  await page.screenshot({ path: process.env.OUT_PNG });

  const snapshot = await page.evaluate(() => {
    function walk(el, idx) {
      const r = el.getBoundingClientRect();
      const s = getComputedStyle(el);
      const visible = s.display !== 'none' && s.visibility !== 'hidden' && r.width > 0 && r.height > 0;
      const children = [];
      let i = 0;
      for (const ch of el.children) children.push(walk(ch, i++));
      return {
        id: el.id || `${el.tagName.toLowerCase()}-${idx}`,
        role: el.getAttribute('role') || el.tagName.toLowerCase(),
        label: (el.innerText || el.getAttribute('aria-label') || '').trim().slice(0, 120) || null,
        value: 'value' in el && typeof el.value === 'string' ? el.value : null,
        bounds: { x: r.x, y: r.y, width: r.width, height: r.height },
        state: {
          visible,
          enabled: !el.disabled,
          focused: document.activeElement === el,
          hovered: el.matches(':hover'),
          pressed: el.getAttribute('aria-pressed') === 'true',
          checked: 'checked' in el ? Boolean(el.checked) : null,
          selected: null
        },
        style_hint: el.className ? String(el.className) : null,
        children
      };
    }
    return {
      protocol_version: 1,
      app_id: document.title || 'web-app',
      toolkit: 'web-dom',
      title: document.title || null,
      viewport: [window.innerWidth, window.innerHeight],
      scale_factor: window.devicePixelRatio || 1,
      timestamp_ms: Date.now(),
      root: walk(document.body, 0)
    };
  });

  let videoPath = null;
  if (recordDir && page.video()) videoPath = await page.video().path();
  await ctx.close();
  await browser.close();
  process.stdout.write(JSON.stringify({ snapshot, video_path: videoPath }) + '\n');
})();
