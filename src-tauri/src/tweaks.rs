/// JS injected into web.whatsapp.com (in addition to the notification shim).
///
///   1. Rebrands UI text "WhatsApp Web" / standalone "WhatsApp" → "OpenWhatsApp"
///   2. Hides the "Download WhatsApp Desktop / Update WhatsApp" banners
///   3. Re-writes the document title (preserving unread counts like "(3) …")
///   4. Adds keyboard shortcuts:
///        Ctrl+W → close the current chat (deselect)
///        Ctrl+Q → quit OpenWhatsApp
///
/// Chat-message contents and other user-generated text are explicitly skipped
/// so we never mangle what a contact actually wrote.
pub const INJECTION_SCRIPT: &str = r##"
(function () {
  "use strict";

  // ── Rebrand ────────────────────────────────────────────────────────────
  function rebrand(text) {
    if (typeof text !== "string" || text.length === 0) return text;
    let out = text.replace(/WhatsApp Web/gi, "OpenWhatsApp");
    // Only rewrite a *standalone* "WhatsApp" so we don't break "WhatsApp Inc."
    // NB: the left-panel header logo is rendered as an SVG, not text — that
    // rename has to happen against the SVG itself, not here.
    if (/^\s*WhatsApp\s*$/.test(out)) out = out.replace(/WhatsApp/, "OpenWhatsApp");
    return out;
  }

  // We must NEVER touch the contents of an actual chat message.
  const SKIP_SELECTORS = [
    "[role='row']",
    "[data-id]",
    ".copyable-text",
    ".selectable-text",
    "input",
    "textarea",
    "[contenteditable='true']",
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

  // ── Banner hider ───────────────────────────────────────────────────────
  const BANNER_PATTERNS = [
    /\bdownload\s+whatsapp\b/i,
    /\bget\s+whatsapp\s+(for|on)\b/i,
    /\btry\s+whatsapp\s+desktop\b/i,
    /\bupdate\s+(your\s+)?whatsapp\b/i,
    /\binstall\s+whatsapp\s+desktop\b/i,
  ];
  function looksLikeBanner(el) {
    if (!el || el.nodeType !== Node.ELEMENT_NODE) return false;
    if (isInsideMessage(el)) return false;
    const t = (el.textContent || "").trim();
    if (t.length === 0 || t.length > 400) return false;
    return BANNER_PATTERNS.some((p) => p.test(t));
  }
  function findBannerRoot(el) {
    let cur = el;
    for (let i = 0; cur && i < 6; i++) {
      const style = (cur.nodeType === Node.ELEMENT_NODE) ? getComputedStyle(cur) : null;
      if (style && (style.position === "fixed" || style.position === "sticky")) return cur;
      const role = cur.getAttribute && cur.getAttribute("role");
      if (role === "banner" || role === "alert" || role === "alertdialog") return cur;
      cur = cur.parentElement;
    }
    return el;
  }
  function killIfBanner(el) {
    if (!looksLikeBanner(el)) return false;
    const root = findBannerRoot(el);
    root.style.setProperty("display", "none", "important");
    return true;
  }
  function sweepBanners(root) {
    if (!root || !root.querySelectorAll) return;
    const cands = root.querySelectorAll(
      "[role='banner'], [role='alert'], [role='alertdialog'], [role='dialog'], div"
    );
    for (const c of cands) {
      if (looksLikeBanner(c)) {
        findBannerRoot(c).style.setProperty("display", "none", "important");
      }
    }
  }

  // ── Video call button hider ────────────────────────────────────────────
  // Both "Video call" and "Group video call" only open an "install desktop
  // app" prompt, so we remove them entirely.
  function hideVideoCallButton() {
    document.querySelectorAll(
      'button[aria-label="Video call"], button[aria-label="Group video call"]'
    ).forEach(function (btn) {
      btn.style.setProperty("display", "none", "important");
    });
  }

  // ── Meta AI button hider ───────────────────────────────────────────────
  // WhatsApp wraps the Meta AI button in several plain <div>s with no
  // semantic roles, so closest('[role="listitem"]') doesn't reach the right
  // level.  Instead we walk up until we find the first ancestor whose parent
  // has multiple children — that ancestor is the "row" div in the nav list.
  // We then hide it AND the preceding sibling (the flex-grow spacer that
  // pushes bottom-nav items downward), which removes the hover ghost and gap.
  function hideMetaAI() {
    document.querySelectorAll('[aria-label="Meta AI"]').forEach(function (el) {
      el.style.setProperty("display", "none", "important");

      // Climb up to the row-level container.
      let row = null;
      let cur = el.parentElement;
      for (let i = 0; i < 12 && cur && cur.parentElement; i++) {
        if (cur.parentElement.children.length >= 3) {
          row = cur;
          break;
        }
        cur = cur.parentElement;
      }

      if (row) {
        row.style.setProperty("display", "none", "important");
        // The flex-grow spacer immediately above the Meta AI row.
        const spacer = row.previousElementSibling;
        if (spacer) spacer.style.setProperty("display", "none", "important");
      }
    });
  }

  // ── Accent colour repainting ───────────────────────────────────────────
  // Swap WhatsApp green out for the blue from the icon gradient midpoint.
  // WhatsApp's CSS arrives in several waves and several forms:
  //
  //   1. Initial <style> tags injected at parse time
  //   2. <style> tags added later by chunk loader / dark-mode swap
  //   3. Existing <style> tags whose text content is rewritten in place
  //   4. CSSOM rules updated via insertRule / replaceSync
  //   5. CSS custom properties redefined at runtime via JS
  //
  // To cover all five we (a) patch <style> textContent, (b) observe
  // childList + characterData mutations, (c) walk cssRules to rewrite CSS
  // variable definitions, and (d) re-sweep every 2 s as a safety net.
  //
  // Colour mapping:
  //   #1daa61  primary green   → #4663c2  primary blue
  //   #00a884  dark-mode green → #4663c2
  //   #1fdf6d  bright green    → #6882d4  lighter hover/active tint
  //   #1ed97e  lighter accent  → #6882d4
  //   #0ded64  brightest green → #7b96df
  const ACCENT_PAIRS = [
    [/#1daa61/gi,                                "#4663c2"],
    [/#00a884/gi,                                "#4663c2"],
    [/#1fdf6d/gi,                                "#6882d4"],
    [/#1ed97e/gi,                                "#6882d4"],
    [/#0ded64/gi,                                "#7b96df"],
    [/rgb\(\s*29\s*,\s*170\s*,\s*97\s*\)/gi,     "rgb(70,99,194)"],
    [/rgba\(\s*29\s*,\s*170\s*,\s*97\s*,/gi,     "rgba(70,99,194,"],
  ];
  function patchText(t) {
    if (!t) return t;
    let out = t;
    for (const [re, rep] of ACCENT_PAIRS) out = out.replace(re, rep);
    return out;
  }
  function patchStyleEl(el) {
    if (!el || el.nodeName !== "STYLE") return;
    const next = patchText(el.textContent);
    if (next !== el.textContent) el.textContent = next;
  }
  // Walk a CSSRuleList recursively (handles @media, @supports, etc.) and
  // rewrite CSS custom-property declarations whose value contains the green.
  function walkCssRules(rules) {
    if (!rules) return;
    for (const rule of rules) {
      if (rule.cssRules) walkCssRules(rule.cssRules);
      const style = rule.style;
      if (!style) continue;
      for (let i = 0; i < style.length; i++) {
        const prop = style[i];
        const val = style.getPropertyValue(prop);
        const next = patchText(val);
        if (next !== val) {
          style.setProperty(prop, next, style.getPropertyPriority(prop));
        }
      }
    }
  }
  function repaintAccent() {
    document.querySelectorAll("style").forEach(patchStyleEl);
    for (const sheet of document.styleSheets) {
      try { walkCssRules(sheet.cssRules); } catch (_) { /* cross-origin */ }
    }
    if (document.adoptedStyleSheets) {
      for (const sheet of document.adoptedStyleSheets) {
        try { walkCssRules(sheet.cssRules); } catch (_) {}
      }
    }
  }
  function startAccentRepainter() {
    if (!document.documentElement) { setTimeout(startAccentRepainter, 10); return; }
    repaintAccent();
    new MutationObserver(function (mutations) {
      let needsSweep = false;
      for (const m of mutations) {
        if (m.type === "childList") {
          for (const n of m.addedNodes) {
            if (n.nodeName === "STYLE") { patchStyleEl(n); needsSweep = true; }
          }
        } else if (m.type === "characterData") {
          // Text inside an existing <style> was rewritten in place.
          const p = m.target && m.target.parentNode;
          if (p && p.nodeName === "STYLE") { patchStyleEl(p); needsSweep = true; }
        }
      }
      if (needsSweep) repaintAccent();
    }).observe(document.documentElement, {
      childList: true, subtree: true, characterData: true,
    });
    // Safety-net re-sweep: catches CSS variables redefined via JS or rules
    // mutated via insertRule/replaceSync (which don't fire DOM mutations).
    setInterval(repaintAccent, 2000);
  }
  startAccentRepainter();

  // ── Title rewrite (preserve "(3) " unread prefix) ──────────────────────
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
    hideVideoCallButton();
    hideMetaAI();
    rebrandTitle();

    const mo = new MutationObserver((mutations) => {
      for (const m of mutations) {
        for (const added of m.addedNodes) {
          if (added.nodeType === Node.ELEMENT_NODE) {
            walkText(added);
            if (!killIfBanner(added)) sweepBanners(added);
            hideVideoCallButton();
            hideMetaAI();
          } else if (added.nodeType === Node.TEXT_NODE) {
            walkText(added);
          }
        }
        if (m.type === "characterData") walkText(m.target);
      }
    });
    mo.observe(document.body, { childList: true, subtree: true, characterData: true });

    const titleEl = document.querySelector("title");
    if (titleEl) {
      new MutationObserver(rebrandTitle).observe(titleEl, {
        childList: true, characterData: true, subtree: true,
      });
    }
    setInterval(function () {
      rebrandTitle();
      hideVideoCallButton();
      hideMetaAI();
    }, 2000);
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

    // Ctrl+W → close / deselect the current chat
    if (k === "w" && !e.shiftKey) {
      e.preventDefault();
      e.stopImmediatePropagation();
      fireEsc();
      // Some WhatsApp surfaces (image viewer, profile, etc.) have explicit X
      const closeIcon = document.querySelector(
        "header [data-icon='x'], [data-icon='x-viewer'], [aria-label='Close']"
      );
      if (closeIcon) { try { closeIcon.click(); } catch (_) {} }
      // SPA fallback: push base URL so the right pane re-empties
      try {
        window.history.pushState({}, "", "/");
        window.dispatchEvent(new PopStateEvent("popstate"));
      } catch (_) {}
      return;
    }

    // Ctrl+Q → quit
    if (k === "q" && !e.shiftKey) {
      e.preventDefault();
      e.stopImmediatePropagation();
      try { window.__TAURI_INTERNALS__.invoke("quit_app"); } catch (_) {}
      return;
    }

    // Ctrl+P → swallow (no print dialog from a chat app).
    if (k === "p") {
      e.preventDefault();
      e.stopImmediatePropagation();
      return;
    }
  }, true);  // capture so we beat WhatsApp's own handlers
})();
"##;

/// Extra injection only used in release builds.  Disables the WebView2 native
/// right-click menu (Reload / Inspect / etc.) while leaving WhatsApp's own
/// JS-rendered context menus intact — they still fire on the same event, we
/// just stop the browser's *default action* of showing the OS-level menu.
#[allow(dead_code)] // referenced only in release builds via #[cfg(not(debug_assertions))]
pub const PROD_INJECTION_SCRIPT: &str = r#"
(function () {
  document.addEventListener("contextmenu", function (e) {
    e.preventDefault();
  }, false);
})();
"#;
