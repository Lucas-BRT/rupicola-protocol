// Helpers the CDP backend evaluates in the page: find an element by the same kinds of
// target a plan may name for the Playwright backend, and ask about it.

/** The element a target names: `css=`, `xpath=`, `text=` or `id=`, a selector that starts
 *  with `.`, `#`, `[` or `//`, or the id of an element. */
function cdpResolve(target) {
  if (/^css=/.test(target)) return document.querySelector(target.slice(4));
  if (/^xpath=/.test(target) || /^\/\//.test(target)) {
    const path = target.replace(/^xpath=/, '');
    return document.evaluate(path, document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null)
      .singleNodeValue;
  }
  if (/^text=/.test(target)) {
    const wanted = target.slice(5).trim().toLowerCase();
    // The deepest element that holds the text, so a container is not picked over its label.
    let found = null;
    for (const el of document.querySelectorAll('body *')) {
      if ((el.textContent || '').toLowerCase().includes(wanted)) found = el;
    }
    return found;
  }
  if (/^id=/.test(target)) return document.getElementById(target.slice(3));
  if (/^role=/.test(target)) throw new Error('role selectors are not supported by this backend');
  if (/^[[.#]/.test(target)) return document.querySelector(target);
  return document.getElementById(target);
}

/** Whether the element a target names is on screen. */
function cdpVisible(target) {
  const el = cdpResolve(target);
  if (!el) return false;
  const box = el.getBoundingClientRect();
  const style = getComputedStyle(el);
  return box.width > 0 && box.height > 0
    && style.visibility !== 'hidden' && style.display !== 'none';
}

/** Whether some text is on screen. */
function cdpHasText(text) {
  return (document.body.innerText || '').includes(text);
}

/** Whether the page has finished loading. */
function cdpLoaded() {
  return document.readyState === 'complete';
}

function cdpRequire(target) {
  const el = cdpResolve(target);
  if (!el) throw new Error("missing target: " + target);
  return el;
}

function cdpWaitForViewTransition(limitMs) {
  return new Promise((resolve) => {
    const started = performance.now();
    const tick = () => {
      const active = document.getAnimations().some((animation) => {
        const pseudoElement = animation.effect?.pseudoElement;
        return typeof pseudoElement === "string"
          && pseudoElement.startsWith("::view-transition")
          && (animation.playState === "running" || animation.playState === "pending");
      });
      if (!active || performance.now() - started > limitMs) {
        resolve(true);
      } else {
        requestAnimationFrame(tick);
      }
    };
    tick();
  });
}

function cdpCenter(target) {
  const el = cdpRequire(target);
  el.scrollIntoView({ block: "nearest", inline: "nearest" });
  const r = el.getBoundingClientRect();
  if (r.width <= 0 || r.height <= 0) throw new Error("target has zero size: " + target);
  return { x: r.left + r.width / 2, y: r.top + r.height / 2 };
}

function cdpFocus(target, clearExisting) {
  const el = cdpRequire(target);
  el.focus();
  if (clearExisting && "value" in el) {
    el.value = "";
    el.dispatchEvent(new Event("input", { bubbles: true }));
  }
  return true;
}

function cdpScroll(target, dx, dy) {
  const el = cdpRequire(target);
  const scrolls = (node, size, client, overflow) =>
    node[size] > node[client] && /(auto|scroll)/.test(getComputedStyle(node)[overflow]);
  let box = el;
  while (
    box &&
    box !== document.body &&
    box !== document.documentElement &&
    !(dy !== 0 && scrolls(box, "scrollHeight", "clientHeight", "overflowY")) &&
    !(dx !== 0 && scrolls(box, "scrollWidth", "clientWidth", "overflowX"))
  ) {
    box = box.parentElement;
  }
  if (!box || box === document.body || box === document.documentElement) {
    box = document.scrollingElement;
  }
  box.scrollBy(dx, dy);
  return true;
}

function cdpScrollIntoView(target) {
  const el = cdpRequire(target);
  el.scrollIntoView({ block: "nearest", inline: "nearest" });
  return true;
}

function cdpScrollTo(target, x, y) {
  const el = target ? cdpRequire(target) : document.scrollingElement;
  const box = el === document.documentElement ? document.scrollingElement : el;
  box.scrollTo(x ?? box.scrollLeft, y ?? box.scrollHeight);
  return true;
}

function cdpSettle(stableFrames, limitMs) {
  return new Promise((resolve) => {
    const started = performance.now();
    let sawAnimation = false;
    let scrolled = false;
    let last = [window.scrollX, window.scrollY];
    let still = 0;
    const onScroll = () => { scrolled = true; };
    addEventListener("scroll", onScroll, true);
    const tick = () => {
      const animationActive = document.getAnimations().some((animation) => {
        const timing = animation.effect?.getComputedTiming();
        const currentTime = animation.currentTime;
        return timing
          && Number.isFinite(timing.endTime)
          && (animation.playState === "running" || animation.playState === "pending")
          && (currentTime === null || timing.endTime > currentTime);
      });
      sawAnimation ||= animationActive;
      const now = [window.scrollX, window.scrollY];
      if (scrolled || now[0] !== last[0] || now[1] !== last[1]) {
        still = 0;
        last = now;
        scrolled = false;
      } else {
        still += 1;
      }
      const maxWaitMs = sawAnimation ? Math.max(limitMs, 1500) : limitMs;
      if ((!animationActive && still >= stableFrames) || performance.now() - started > maxWaitMs) {
        removeEventListener("scroll", onScroll, true);
        resolve(true);
      } else {
        requestAnimationFrame(tick);
      }
    };
    requestAnimationFrame(tick);
  });
}
