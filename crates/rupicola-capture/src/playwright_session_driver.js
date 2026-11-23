// Long-lived Playwright driver for the capture session contract. Reads one JSON command
// per line on stdin and answers each with one JSON line on stdout:
//   {"ok":true, ...}  or  {"ok":false,"class":"environment"|"target","error":"..."}
const readline = require('readline');

/** Longest a wait condition may take, in milliseconds. */
const WAIT_TIMEOUT_MS = 10000;
/** Pause between recorded frames, in milliseconds. */
const FRAME_INTERVAL_MS = 100;
/** JPEG quality of recorded frames. */
const FRAME_QUALITY = 80;

let pw = null;
let browser = null;
let context = null;
let page = null;
let recording = null;
let waitTimeout = WAIT_TIMEOUT_MS;
let frameInterval = FRAME_INTERVAL_MS;

/** A target is a Playwright selector when it says so, and the id of an element otherwise. */
const selector = (id) =>
  /^(css|text|xpath|role|id)=|^\/\/|^[[.#]/.test(id) ? id : '#' + id;

class DriverError extends Error {
  constructor(errorClass, message) {
    super(message);
    this.errorClass = errorClass;
  }
}

async function launch(request) {
  try {
    pw = require(process.env.PW_DIR);
    const launcher = pw[request.engine] || pw.chromium;
    browser = await launcher.launch();
  } catch (error) {
    throw new DriverError('environment', `cannot start the browser: ${error.message}`);
  }
  context = await browser.newContext({
    viewport: { width: request.viewport[0], height: request.viewport[1] },
    deviceScaleFactor: request.scale,
    colorScheme: request.color_scheme || 'no-preference',
    reducedMotion: request.reduced_motion ? 'reduce' : 'no-preference',
    ...(request.locale ? { locale: request.locale } : {}),
  });
  page = await context.newPage();
  if (request.timeout_ms) {
    waitTimeout = request.timeout_ms;
    page.setDefaultTimeout(request.timeout_ms);
  }
  if (request.frame_interval_ms) frameInterval = request.frame_interval_ms;
  try {
    await page.goto(request.url, { waitUntil: 'networkidle' });
  } catch (error) {
    throw new DriverError('target', `cannot open ${request.url}: ${error.message}`);
  }
  if (request.warmup_ms > 0) await page.waitForTimeout(request.warmup_ms);
}

async function perform(action) {
  switch (action.action) {
    case 'type-text':
      return page.fill(selector(action.target_id), action.text);
    case 'click':
      return page.click(selector(action.target_id));
    case 'click-at':
      return page.mouse.click(action.x, action.y);
    case 'hover':
      return page.hover(selector(action.target_id));
    case 'focus':
      return page.focus(selector(action.target_id));
    case 'press-key':
      return page.keyboard.press([...(action.modifiers || []), action.key].join('+'));
    case 'scroll':
      if (action.target_id) await page.hover(selector(action.target_id));
      return page.mouse.wheel(action.dx, action.dy);
    case 'wait-ms':
      return page.waitForTimeout(action.ms);
    default:
      throw new Error(`unknown action ${action.action}`);
  }
}

async function waitFor(condition) {
  switch (condition.wait) {
    case 'delay':
      return page.waitForTimeout(condition.ms);
    case 'visible':
      return page.waitForSelector(selector(condition.target_id), {
        state: 'visible',
        timeout: waitTimeout,
      });
    case 'text':
      return page.getByText(condition.text).first().waitFor({
        state: 'visible',
        timeout: waitTimeout,
      });
    default:
      throw new Error(`unknown wait ${condition.wait}`);
  }
}

function snapshot() {
  return page.evaluate(() => {
    // The ARIA role when the element has one; otherwise what the element is: an input by its
    // type, anything else by its tag.
    function roleOf(el) {
      const explicit = el.getAttribute('role');
      if (explicit) return explicit;
      const tag = el.tagName.toLowerCase();
      if (tag !== 'input') return tag;
      const byType = {
        checkbox: 'checkbox', radio: 'radio', range: 'slider',
        button: 'button', submit: 'button', reset: 'button', image: 'button',
      };
      return byType[(el.getAttribute('type') || 'text').toLowerCase()] || 'textbox';
    }

    function walk(el, idx) {
      const r = el.getBoundingClientRect();
      const s = getComputedStyle(el);
      const visible =
        s.display !== 'none' && s.visibility !== 'hidden' && r.width > 0 && r.height > 0;
      const children = [];
      let i = 0;
      for (const child of el.children) children.push(walk(child, i++));
      return {
        id: el.id || `${el.tagName.toLowerCase()}-${idx}`,
        role: roleOf(el),
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
          selected: null,
        },
        style_hint: el.className ? String(el.className) : null,
        children,
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
      root: walk(document.body, 0),
    };
  });
}

function startRecording(dir) {
  if (recording) throw new DriverError('environment', 'a recording is already running');
  const state = { running: true, frames: [] };
  state.loop = (async () => {
    let index = 0;
    while (state.running) {
      const started = Date.now();
      const file = `${dir}/frame${String(index++).padStart(6, '0')}.jpg`;
      await page.screenshot({ path: file, type: 'jpeg', quality: FRAME_QUALITY });
      state.frames.push({ file, ms: started });
      await page.waitForTimeout(Math.max(0, frameInterval - (Date.now() - started)));
    }
  })();
  recording = state;
}

async function stopRecording() {
  if (!recording) throw new DriverError('environment', 'no recording is running');
  const state = recording;
  recording = null;
  state.running = false;
  await state.loop;
  return state.frames;
}

async function handle(request) {
  switch (request.cmd) {
    case 'launch':
      return launch(request);
    case 'perform':
      return perform(request.action);
    case 'wait':
      return waitFor(request.condition);
    case 'screenshot':
      await page.screenshot({ path: request.path });
      return request.tree ? { snapshot: await snapshot() } : {};
    case 'start-recording':
      return startRecording(request.dir);
    case 'stop-recording':
      return { frames: await stopRecording() };
    case 'close':
      if (recording) await stopRecording();
      if (browser) await browser.close();
      return {};
    default:
      throw new Error(`unknown command ${request.cmd}`);
  }
}

const lines = readline.createInterface({ input: process.stdin });
let queue = Promise.resolve();
lines.on('line', (line) => {
  // Commands run strictly one after another.
  queue = queue.then(async () => {
    let reply;
    let command = null;
    try {
      const request = JSON.parse(line);
      command = request.cmd;
      reply = { ok: true, ...((await handle(request)) || {}) };
    } catch (error) {
      reply = {
        ok: false,
        class: error.errorClass || 'target',
        error: String(error.message || error),
      };
    }
    process.stdout.write(JSON.stringify(reply) + '\n');
    if (reply.ok && command === 'close') process.exit(0);
  });
});
lines.on('close', () => {
  queue.then(() => (browser ? browser.close() : null)).finally(() => process.exit(0));
});
