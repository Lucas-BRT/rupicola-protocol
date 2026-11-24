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
