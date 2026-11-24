// Walks the DOM of the page it runs in and returns its widget tree, in the shape of the
// protocol's `UiSnapshot`. It takes no arguments and uses nothing outside the page, so a
// driver can hand it to a browser to run (`page.evaluate(domSnapshot)`, or the text of this
// file followed by `domSnapshot()` through the Chrome DevTools Protocol).
function domSnapshot() {
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
}
