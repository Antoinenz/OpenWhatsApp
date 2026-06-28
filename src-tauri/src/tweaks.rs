/// JS injected into web.whatsapp.com (in addition to the notification shim).
///
///   1. Stubs the Electron runtime (window.require / window.process / window.electron)
///      so WhatsApp Web's Electron code-path initialises cleanly inside WebView2.
///   2. Spoofs navigator.userAgent to include WhatsApp/Electron tokens (preserving
///      the real Chrome version so the server never shows "Update Chrome").
///   3. Rebrands UI text "WhatsApp Web" / standalone "WhatsApp" → "OpenWhatsApp"
///   4. Hides every flavour of "Download / Get / Install / Try WhatsApp Desktop"
///      banner *including* the modal dialog that pops up when you click Call.
///   5. Re-writes the document title (preserving unread counts like "(3) …")
///   6. Adds keyboard shortcuts:
///        Ctrl+W → close the current chat (deselect)
///        Ctrl+Q → quit OpenWhatsApp
///
/// Chat-message contents and other user-generated text are explicitly skipped
/// so we never mangle what a contact actually wrote.
pub const INJECTION_SCRIPT: &str = r#"
(function () {
  "use strict";

  // ── Electron runtime stub ──────────────────────────────────────────────
  // WhatsApp Web detects Electron via window.process.type === 'renderer' and
  // window.require('electron').  We provide a complete-enough fake so the app
  // initialises on the Electron code-path without crashing on missing APIs.
  //
  // Key design choices:
  //   • ipcRenderer: all channels are silent no-ops / resolved Promises.
  //     WhatsApp's React router owns its own navigation state; IPC calls for
  //     chat routing are side-effects (native title bar, etc.) — safe to drop.
  //   • shell.openExternal → window.open (so external links still open)
  //   • BrowserWindow.close → our quit_app Tauri command
  //   • Everything else returns a neutral default.
  (function () {
    // ── process ───────────────────────────────────────────────────────────
    var chromeVer = (navigator.userAgent.match(/Chrome\/([\d.]+)/) || [])[1] || "120.0.0.0";
    try {
      if (!window.process || typeof window.process.type === "undefined") {
        Object.defineProperty(window, "process", {
          value: {
            type:     "renderer",
            platform: "win32",
            versions: { electron: "32.0.0", chrome: chromeVer, node: "20.18.0" },
            env:      {},
            argv:     [],
            execPath: "",
            pid:      1,
          },
          writable: true, configurable: true,
        });
      }
    } catch (_) {}

    // ── ipcRenderer ───────────────────────────────────────────────────────
    var _listeners = Object.create(null);
    var ipcRenderer = {
      send:               function () {},
      sendSync:           function () { return undefined; },
      sendToHost:         function () {},
      postMessage:        function () {},
      invoke:             function () { return Promise.resolve(null); },
      on: function (ch, fn) {
        (_listeners[ch] = _listeners[ch] || []).push(fn); return this;
      },
      once: function (ch, fn) {
        var w = function () { ipcRenderer.removeListener(ch, w); fn.apply(this, arguments); };
        return this.on(ch, w);
      },
      removeListener: function (ch, fn) {
        var a = _listeners[ch]; if (!a) return this;
        var i = a.indexOf(fn); if (i !== -1) a.splice(i, 1); return this;
      },
      removeAllListeners: function (ch) {
        if (ch) delete _listeners[ch];
        else Object.keys(_listeners).forEach(function (k) { delete _listeners[k]; });
        return this;
      },
      eventNames: function () { return Object.keys(_listeners); },
    };

    // ── shell ─────────────────────────────────────────────────────────────
    var shell = {
      openExternal: function (url) {
        try { window.open(url, "_blank", "noopener,noreferrer"); } catch (_) {}
        return Promise.resolve();
      },
      openPath:         function () { return Promise.resolve(""); },
      showItemInFolder: function () {},
      beep:             function () {},
    };

    // ── nativeImage ───────────────────────────────────────────────────────
    function fakeImg() {
      return { isEmpty: function () { return true; }, toDataURL: function () { return ""; },
               getSize: function () { return { width: 0, height: 0 }; } };
    }
    var nativeImage = {
      createFromDataURL: fakeImg, createEmpty: fakeImg, createFromPath: fakeImg,
    };

    // ── clipboard ─────────────────────────────────────────────────────────
    var clipboard = {
      readText:         function () { return ""; },
      writeText:        function (t) { try { navigator.clipboard.writeText(t); } catch (_) {} },
      readHTML:         function () { return ""; },
      writeHTML:        function () {},
      clear:            function () {},
      availableFormats: function () { return []; },
    };

    // ── BrowserWindow (current window proxy) ──────────────────────────────
    function noop() {}
    var fakeWin = {
      minimize:       noop, maximize: noop, unmaximize: noop,
      restore:        noop, hide: noop, show: noop, focus: noop,
      isMaximized:    function () { return false; },
      isMinimized:    function () { return false; },
      isFullScreen:   function () { return false; },
      isVisible:      function () { return true; },
      setTitle:       noop, getTitle: function () { return document.title; },
      setProgressBar: noop, setOverlayIcon: noop, flashFrame: noop,
      close:          function () {
        try { window.__TAURI_INTERNALS__.invoke("quit_app"); } catch (_) {}
      },
      webContents: {
        send:              noop,
        executeJavaScript: function () { return Promise.resolve(); },
        getURL:            function () { return location.href; },
        getUserAgent:      function () { return navigator.userAgent; },
      },
      on: function () { return this; },
      once: function () { return this; },
      removeListener: function () { return this; },
    };

    // ── app ───────────────────────────────────────────────────────────────
    var app = {
      getVersion:           function () { return "2.2422.6"; },
      getName:              function () { return "WhatsApp"; },
      getPath:              function () { return ""; },
      getLocale:            function () { return navigator.language || "en-US"; },
      getLocaleCountryCode: function () { return (navigator.language || "en-US").split("-")[1] || "US"; },
      isPackaged:           true,
      on: function () { return this; },
      once: function () { return this; },
      removeListener: function () { return this; },
      quit:           function () {
        try { window.__TAURI_INTERNALS__.invoke("quit_app"); } catch (_) {}
      },
    };

    // ── remote (legacy) ───────────────────────────────────────────────────
    var remote = {
      app:              app,
      shell:            shell,
      nativeImage:      nativeImage,
      clipboard:        clipboard,
      getCurrentWindow: function () { return fakeWin; },
      getGlobal:        function () { return undefined; },
      require:          function () { return {}; },
      BrowserWindow:    { fromId: function () { return fakeWin; } },
      dialog: {
        showMessageBox:  function () { return Promise.resolve({ response: 0 }); },
        showOpenDialog:  function () { return Promise.resolve({ canceled: true, filePaths: [] }); },
        showSaveDialog:  function () { return Promise.resolve({ canceled: true }); },
      },
    };

    // ── Full electron module ───────────────────────────────────────────────
    var electronModule = {
      ipcRenderer:    ipcRenderer,
      shell:          shell,
      clipboard:      clipboard,
      nativeImage:    nativeImage,
      app:            app,
      remote:         remote,
      contextBridge:  { exposeInMainWorld: noop },
      crashReporter:  { start: noop, getLastCrashReport: function () { return null; } },
      desktopCapturer:{ getSources: function () { return Promise.resolve([]); } },
      systemPreferences: {
        isDarkMode:           function () { return window.matchMedia("(prefers-color-scheme: dark)").matches; },
        getEffectiveAppearance: function () { return "dark"; },
      },
    };

    // ── window.require ────────────────────────────────────────────────────
    try {
      if (!window.require) {
        window.require = function (mod) {
          if (mod === "electron") return electronModule;
          var e = new Error("Cannot find module '" + mod + "'");
          e.code = "MODULE_NOT_FOUND"; throw e;
        };
        window.require.resolve = function () { return ""; };
      }
    } catch (_) {}

    // Some WhatsApp builds expose via contextBridge as window.electron
    try { if (!window.electron) window.electron = electronModule; } catch (_) {}
  })();

  // ── UA spoof ───────────────────────────────────────────────────────────
  // Read the real Chrome version BEFORE we override anything, then splice in
  // WhatsApp/Electron tokens.  The HTTP request UA is untouched — server still
  // sees the real Edge UA so it never serves the "Update Chrome" page.
  try {
    var _realUA = navigator.userAgent || "";
    var _chrome = _realUA.match(/Chrome\/[\d.]+/);
    if (_chrome) {
      var _spoofUA =
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 " +
        "(KHTML, like Gecko) WhatsApp/2.2422.6 " + _chrome[0] +
        " Electron/32.0.0 Safari/537.36";
      var _uaDesc = { get: function () { return _spoofUA; }, configurable: true };
      try { Object.defineProperty(navigator, "userAgent", _uaDesc); } catch (_) {}
      try { Object.defineProperty(Navigator.prototype, "userAgent", _uaDesc); } catch (_) {}
      var _avDesc = { get: function () { return _spoofUA.replace(/^Mozilla\//, ""); }, configurable: true };
      try { Object.defineProperty(navigator, "appVersion", _avDesc); } catch (_) {}
    }
  } catch (_) {}

  // ── Rebrand ────────────────────────────────────────────────────────────
  function rebrand(text) {
    if (typeof text !== "string" || text.length === 0) return text;
    let out = text.replace(/WhatsApp Web/gi, "OpenWhatsApp");
    if (/^\s*WhatsApp\s*$/.test(out)) out = out.replace(/WhatsApp/, "OpenWhatsApp");
    return out;
  }

  const SKIP_SELECTORS = [
    "[role='row']", "[data-id]", ".copyable-text", ".selectable-text",
    "input", "textarea", "[contenteditable='true']",
  ];
  function isInsideMessage(node) {
    const el = node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
    if (!el) return false;
    for (const sel of SKIP_SELECTORS) {
      try { if (el.closest && el.closest(sel)) return true; } catch (_) {}
    }
    return false;
  }

  function walkText(node) {
    if (node.nodeType === Node.TEXT_NODE) {
      if (isInsideMessage(node)) return;
      const t = rebrand(node.textContent);
      if (t !== node.textContent) node.textContent = t;
      return;
    }
    if (node.nodeType !== Node.ELEMENT_NODE) return;
    const tag = node.tagName;
    if (tag === "SCRIPT" || tag === "STYLE") return;
    if (isInsideMessage(node)) return;
    for (const c of node.childNodes) walkText(c);
  }

  // ── Banner / Install-Modal hider ──────────────────────────────────────
  // Patterns are tested case-insensitively against an element's textContent.
  const BANNER_PATTERNS = [
    /\bdownload\s+whatsapp\b/i,
    /\bget\s+whatsapp\b/i,
    /\bget\s+the\s+(whatsapp\s+)?app\b/i,
    /\btry\s+whatsapp\s+desktop\b/i,
    /\bupdate\s+(your\s+)?whatsapp\b/i,
    /\binstall\s+whatsapp\b/i,
    /\buse\s+whatsapp\s+(desktop|for\s+windows)\b/i,
    /\bopen\s+(in|with)\s+whatsapp\s+(desktop|for\s+windows)\b/i,
    /\bcontinue\s+(on|in)\s+whatsapp\s+desktop\b/i,
    /\bswitch\s+to\s+whatsapp\s+desktop\b/i,
    /\bwhatsapp\s+for\s+windows\b/i,
  ];

  function matchesBanner(text) {
    if (!text) return false;
    return BANNER_PATTERNS.some((p) => p.test(text));
  }

  function trim(s) { return (s || "").trim(); }

  /**
   * Walk *up* from a node whose textContent matches a banner pattern, hiding
   * the smallest ancestor that fully encloses the banner *card* without
   * spilling into siblings (other chats, etc.).
   *
   * Stops climbing at:
   *   - the body / html root
   *   - a semantic container tag: <aside>, <main>, <nav>, <header>, <footer>
   *   - a node with an ARIA role that bounds a region: navigation / main /
   *     complementary / list / listbox
   *   - a parent with many children (looks like a virtualised list)
   *   - a parent whose textContent is suddenly very large (entered a wrapper)
   *   - a parent that no longer matches any banner pattern at all
   *
   * Immediately returns at:
   *   - position: fixed / sticky (a floating banner)
   *   - role=banner / alert / alertdialog / dialog (a modal / call-out)
   */
  function findBannerRoot(el) {
    let cur = el;
    let best = el;
    for (let i = 0; cur && cur.parentElement && i < 8; i++) {
      const parent = cur.parentElement;
      if (parent === document.body || parent === document.documentElement) break;

      const tag = parent.tagName;
      if (tag === "ASIDE" || tag === "MAIN" || tag === "NAV" ||
          tag === "HEADER" || tag === "FOOTER") break;

      // Immediate-return signals
      try {
        const style = getComputedStyle(parent);
        const role = parent.getAttribute && parent.getAttribute("role");
        if (style.position === "fixed" || style.position === "sticky") return parent;
        if (role === "banner" || role === "alert" || role === "alertdialog" || role === "dialog") return parent;
        if (role === "navigation" || role === "main" || role === "complementary" ||
            role === "list" || role === "listbox" || role === "grid") break;
      } catch (_) {}

      // Parent must still carry banner-like text.
      if (!matchesBanner(parent.textContent || "")) break;

      // Looks like a virtualised list (many sibling rows).
      if (parent.children.length > 6) break;

      // Wrapper too big — we've definitely left the card.
      if (trim(parent.textContent).length > 700) break;

      best = parent;
      cur = parent;
    }
    return best;
  }

  function hide(el) {
    try { el.style.setProperty("display", "none", "important"); } catch (_) {}
    try { el.style.setProperty("visibility", "hidden", "important"); } catch (_) {}
  }

  function killBanner(el) {
    const root = findBannerRoot(el);
    hide(root);
    return root;
  }

  function sweepBanners(root) {
    if (!root || !root.querySelectorAll) return;
    // Cheap pre-filter: only inspect elements whose own textContent matches.
    // querySelectorAll('*') would be huge — walk via TreeWalker to bail early.
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT, {
      acceptNode(el) {
        if (isInsideMessage(el)) return NodeFilter.FILTER_REJECT;
        if (!matchesBanner(el.textContent || "")) return NodeFilter.FILTER_SKIP;
        // Skip elements whose textContent is just the same as a banner-y child's —
        // we only want the lowest matching node, then `findBannerRoot` walks up.
        for (const c of el.children) {
          if (matchesBanner((c.textContent || ""))) return NodeFilter.FILTER_SKIP;
        }
        return NodeFilter.FILTER_ACCEPT;
      },
    });
    const hits = [];
    let n;
    while ((n = walker.nextNode())) hits.push(n);
    for (const h of hits) killBanner(h);
  }

  /**
   * Special-case: the "Install WhatsApp for Windows to make calls" modal.
   * It's a full-screen dialog with backdrop; we kill the dialog AND walk
   * out to the overlay so the backdrop disappears too.
   */
  function killInstallDialogs(root) {
    root = root || document.body;
    if (!root.querySelectorAll) return;
    const dialogs = root.querySelectorAll('[role="dialog"], [role="alertdialog"]');
    for (const d of dialogs) {
      const txt = (d.textContent || "").toLowerCase();
      if (matchesBanner(txt)) {
        hide(d);
        // Walk out to the fixed-position overlay (backdrop) and hide it too.
        let p = d.parentElement;
        for (let i = 0; p && i < 6 && p !== document.body; i++) {
          try {
            const st = getComputedStyle(p);
            if (st.position === "fixed" || st.position === "absolute") { hide(p); break; }
          } catch (_) {}
          p = p.parentElement;
        }
      }
    }
  }

  // ── Title rewrite ──────────────────────────────────────────────────────
  function rebrandTitle() {
    const t = document.title || "";
    let n = t.replace(/WhatsApp Web/gi, "OpenWhatsApp");
    if (/^\s*WhatsApp\s*$/.test(n)) n = "OpenWhatsApp";
    n = n.replace(/(\(\d+\)\s*)WhatsApp(?!\w)/g, "$1OpenWhatsApp");
    if (n !== t) document.title = n;
  }

  // ── Boot ───────────────────────────────────────────────────────────────
  function boot() {
    if (!document.body) { setTimeout(boot, 50); return; }

    walkText(document.body);
    sweepBanners(document.body);
    killInstallDialogs(document.body);
    rebrandTitle();

    const mo = new MutationObserver((mutations) => {
      const dirtyRoots = new Set();
      for (const m of mutations) {
        for (const added of m.addedNodes) {
          if (added.nodeType === Node.ELEMENT_NODE) {
            walkText(added);
            dirtyRoots.add(added);
          } else if (added.nodeType === Node.TEXT_NODE) {
            walkText(added);
            if (added.parentElement) dirtyRoots.add(added.parentElement);
          }
        }
        if (m.type === "characterData") walkText(m.target);
      }
      for (const r of dirtyRoots) {
        sweepBanners(r);
        killInstallDialogs(r);
      }
    });
    mo.observe(document.body, { childList: true, subtree: true, characterData: true });

    const titleEl = document.querySelector("title");
    if (titleEl) {
      new MutationObserver(rebrandTitle).observe(titleEl, {
        childList: true, characterData: true, subtree: true,
      });
    }
    // Safety net for SPA transitions that don't touch document.body directly.
    setInterval(() => { rebrandTitle(); killInstallDialogs(document.body); }, 1500);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", boot, { once: true });
  } else {
    boot();
  }

  // ── Keyboard shortcuts ─────────────────────────────────────────────────
  function fireEsc() {
    const opts = { key: "Escape", code: "Escape", keyCode: 27, which: 27,
                   bubbles: true, cancelable: true };
    const target = document.activeElement || document.body;
    target.dispatchEvent(new KeyboardEvent("keydown", opts));
    target.dispatchEvent(new KeyboardEvent("keyup", opts));
  }

  document.addEventListener("keydown", (e) => {
    if (!e.ctrlKey || e.altKey || e.metaKey) return;
    const k = e.key.toLowerCase();

    if (k === "w" && !e.shiftKey) {
      e.preventDefault();
      e.stopImmediatePropagation();
      fireEsc();
      const closeIcon = document.querySelector(
        "header [data-icon='x'], [data-icon='x-viewer'], [aria-label='Close']"
      );
      if (closeIcon) { try { closeIcon.click(); } catch (_) {} }
      try {
        window.history.pushState({}, "", "/");
        window.dispatchEvent(new PopStateEvent("popstate"));
      } catch (_) {}
      return;
    }

    if (k === "q" && !e.shiftKey) {
      e.preventDefault();
      e.stopImmediatePropagation();
      try { window.__TAURI_INTERNALS__.invoke("quit_app"); } catch (_) {}
      return;
    }
  }, true);
})();
"#;
