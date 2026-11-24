// Long-lived Playwright driver for the capture session contract. Reads one JSON command
// per line on stdin and answers each with one JSON line on stdout:
//   {"ok":true, ...}  or  {"ok":false,"class":"environment"|"target","error":"..."}
const readline = require('readline');
const dns = require('dns').promises;
const net = require('net');

/** Longest a wait condition may take, in milliseconds. */
const WAIT_TIMEOUT_MS = 10000;
/** Pause between recorded frames, in milliseconds. */
const FRAME_INTERVAL_MS = 100;
/** Frames in a row the page must show no scrolling before an action counts as done. */
const SETTLE_STABLE_FRAMES = 3;
/** Longest an action may take to settle, in milliseconds. */
const SETTLE_LIMIT_MS = 3000;
/** JPEG quality of recorded frames. */
const FRAME_QUALITY = 80;

let pw = null;
let browser = null;
let context = null;
let page = null;
let recording = null;
let waitTimeout = WAIT_TIMEOUT_MS;
let frameInterval = FRAME_INTERVAL_MS;
let settleMs = 0;

/** A target is a Playwright selector when it says so, and the id of an element otherwise. */
const selector = (id) =>
  /^(css|text|xpath|role|id)=|^\/\/|^[[.#]/.test(id) ? id : '#' + id;

class DriverError extends Error {
  constructor(errorClass, message) {
    super(message);
    this.errorClass = errorClass;
  }
}

function isForbiddenIpv4(ip) {
  const parts = ip.split('.').map(Number);
  if (parts.length !== 4 || parts.some((n) => Number.isNaN(n) || n < 0 || n > 255)) return true;
  const [a, b] = parts;
  return (
    a === 0 ||
    a === 10 ||
    a === 127 ||
    (a === 100 && b >= 64 && b <= 127) ||
    (a === 169 && b === 254) ||
    (a === 172 && b >= 16 && b <= 31) ||
    (a === 192 && b === 168) ||
    a >= 224
  );
}

function isForbiddenIp(rawIp) {
  const ip = rawIp.toLowerCase();
  if (net.isIPv4(ip)) return isForbiddenIpv4(ip);
  if (!net.isIPv6(ip)) return true;
  if (ip === '::1' || ip === '::' || ip.startsWith('fe80:') || ip.startsWith('fc') || ip.startsWith('fd') || ip.startsWith('ff')) {
    return true;
  }
  if (ip.startsWith('::ffff:')) {
    const mapped = ip.slice('::ffff:'.length);
    if (net.isIPv4(mapped)) return isForbiddenIpv4(mapped);
  }
  return false;
}

async function isUrlAllowed(rawUrl) {
  let parsed;
  try {
    parsed = new URL(rawUrl);
  } catch {
    return false;
  }
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') return false;
  if (parsed.username || parsed.password) return false;
  const host = parsed.hostname.replace(/\.$/, '').toLowerCase();
  if (!host || host === 'localhost' || host.endsWith('.localhost')) return false;
  if (net.isIP(host)) return !isForbiddenIp(host);
  try {
    const records = await dns.lookup(host, { all: true });
    if (!records || records.length === 0) return false;
    return records.every((record) => !isForbiddenIp(record.address));
  } catch {
    return false;
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
  if (request.network_policy === 'public-only') {
    await context.route('**/*', async (route) => {
      if (await isUrlAllowed(route.request().url())) {
        await route.continue();
      } else {
        await route.abort('blockedbyclient');
      }
    });
  }
  page = await context.newPage();
  if (request.timeout_ms) {
    waitTimeout = request.timeout_ms;
    page.setDefaultTimeout(request.timeout_ms);
  }
  if (request.frame_interval_ms) frameInterval = request.frame_interval_ms;
  settleMs = request.settle_ms || 0;
  try {
    await page.goto(request.url, { waitUntil: 'networkidle' });
  } catch (error) {
    throw new DriverError('target', `cannot open ${request.url}: ${error.message}`);
  }
  if (request.warmup_ms > 0) await page.waitForTimeout(request.warmup_ms);
}

/**
 * Resolves once the page has painted a few frames without scrolling: neither the window nor
 * any element moved. Keys, wheel turns and clicks scroll a frame or more after they return,
 * so a screenshot taken right away would show the page before the action.
 */
function settle() {
  return page.evaluate(
    ([stableFrames, limitMs]) =>
      new Promise((resolve) => {
        const started = performance.now();
        let scrolled = false;
        let last = [scrollX, scrollY];
        let still = 0;
        const onScroll = () => {
          scrolled = true;
        };
        addEventListener('scroll', onScroll, true);
        const tick = () => {
          const now = [scrollX, scrollY];
          if (scrolled || now[0] !== last[0] || now[1] !== last[1]) {
            still = 0;
            last = now;
            scrolled = false;
          } else {
            still += 1;
          }
          if (still >= stableFrames || performance.now() - started > limitMs) {
            removeEventListener('scroll', onScroll, true);
            resolve();
          } else {
            requestAnimationFrame(tick);
          }
        };
        requestAnimationFrame(tick);
      }),
    [SETTLE_STABLE_FRAMES, SETTLE_LIMIT_MS],
  );
}

/** Scrolls an element, or the page, to `x` and `y`; a `null` is the current left, or the end. */
const SCROLL_TO_JS = `(el, [x, y]) => {
  const box = el || document.scrollingElement;
  box.scrollTo(x ?? box.scrollLeft, y ?? box.scrollHeight);
}`;

function scrollTo(id, x, y) {
  if (id) return page.locator(selector(id)).first().evaluate(SCROLL_TO_JS, [x, y]);
  return page.evaluate(`(${SCROLL_TO_JS})(null, ${JSON.stringify([x, y])})`);
}

/** Performs `action`, then waits until its effect is on screen. */
async function perform(action) {
  await act(action);
  if (action.action === 'wait-ms') return;
  await settle();
  if (settleMs > 0) await page.waitForTimeout(settleMs);
}

async function act(action) {
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
    case 'scroll-into-view':
      return page.locator(selector(action.target_id)).first().scrollIntoViewIfNeeded();
    case 'scroll-to':
      return scrollTo(action.target_id, action.x, action.y);
    case 'scroll-to-end':
      return scrollTo(action.target_id, null, null);
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
  return page.evaluate(domSnapshot);
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
      await page.screenshot({ path: request.path, fullPage: request.full_page });
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
