// Draws the floating "chat-head" overlay for the Messenger window. The window
// is transparent and decorationless; in bubble mode Rust shrinks it to ~76px and
// this overlay paints an opaque circle (Messenger logo + unread badge) over the
// clipped page. In full mode the overlay is hidden so the page is fully usable.
//
// State is pushed from Rust via `window.__skSetState('bubble'|'full')` and
// `window.__skSetMuted(bool)` (see bubble.rs). Actions are signalled back through
// the same `swissknife-link://window?action=...` scheme the title bar uses, so
// the remote page never gets IPC/ACL access to real commands. Left-click expands
// (Rust ignores the click if it was the tail of a drag); right-click asks Rust
// to pop a native context menu. Dragging uses `data-tauri-drag-region`.
(function () {
  var ID = "__sk_bubble";
  window.__skMode = window.__skMode || "full";
  window.__skMuted = window.__skMuted || false;

  // Messenger-style speech bubble with a lightning bolt, in white.
  var LOGO =
    '<svg width="30" height="30" viewBox="0 0 24 24" fill="#fff" aria-hidden="true">' +
    '<path d="M12 2.4c-5.4 0-9.6 3.96-9.6 9.3 0 2.82 1.17 5.26 3.08 6.94.16.14.26.34.26.56l.05 1.73c.02.55.59.9 1.09.68l1.93-.85c.17-.08.36-.09.54-.04.88.24 1.82.37 2.75.37 5.4 0 9.6-3.96 9.6-9.3S17.4 2.4 12 2.4z"/>' +
    '<path d="M6.3 14.4l2.82-4.47c.45-.71 1.41-.89 2.08-.38l2.24 1.68c.21.15.49.15.69 0l3.03-2.3c.4-.31.93.18.66.61l-2.82 4.47c-.45.71-1.41.89-2.08.38l-2.24-1.68a.57.57 0 0 0-.69 0l-3.03 2.3c-.4.31-.93-.18-.66-.61z" fill="#0078ff"/>' +
    "</svg>";

  function act(a) {
    window.location.href = "swissknife-link://window?action=" + a;
  }

  // Hide/show the Facebook page's paint via the `sk-page-hidden` class (rules in
  // messenger.css). The overlay forces itself visible, so hiding <html> leaves
  // only the bubble drawn (no page ring around the circle). The page keeps
  // running while hidden, so the unread count stays live. The CSS also kills the
  // scrollbars the huge page would otherwise draw inside the tiny bubble window.
  function setPageHidden(hidden) {
    var de = document.documentElement;
    if (de) de.classList.toggle("sk-page-hidden", hidden);
  }

  function apply(mode) {
    var root = document.getElementById(ID);
    if (!root) return;
    if (mode === "bubble") {
      setPageHidden(true);
      root.classList.add("sk-mounted");
      // Next frame so the opacity transition runs.
      requestAnimationFrame(function () {
        root.classList.add("sk-visible");
      });
      updateBadge();
    } else {
      setPageHidden(false);
      root.classList.remove("sk-visible");
      setTimeout(function () {
        if (window.__skMode !== "bubble") root.classList.remove("sk-mounted");
      }, 180);
    }
  }

  function updateBadge() {
    var badge = document.getElementById(ID + "_badge");
    if (!badge) return;
    var n = 0;
    var m = /^\((\d+)\)/.exec(document.title || "");
    if (m) n = parseInt(m[1], 10) || 0;
    if (window.__skMuted || n <= 0) {
      badge.classList.remove("sk-on");
    } else {
      badge.textContent = n > 99 ? "99+" : String(n);
      badge.classList.add("sk-on");
    }
  }

  window.__skSetState = function (mode) {
    window.__skMode = mode;
    window.dispatchEvent(new CustomEvent("__sk:mode", { detail: mode }));
    apply(mode);
  };
  window.__skSetMuted = function (muted) {
    window.__skMuted = !!muted;
    updateBadge();
  };

  // Plain Esc collapses a full window back to the bubble (same path as the title
  // bar's collapse button). Capture phase so we win before Facebook's own Esc
  // handlers; no-op while already a bubble. Requires NO modifiers so it doesn't
  // double-fire with the configurable Cmd+Esc toggle (a global shortcut handled
  // in Rust, which also works from the bubble and from other apps).
  document.addEventListener(
    "keydown",
    function (e) {
      if (
        e.key === "Escape" &&
        !e.metaKey &&
        !e.ctrlKey &&
        !e.altKey &&
        !e.shiftKey &&
        window.__skMode !== "bubble"
      ) {
        e.preventDefault();
        e.stopPropagation();
        act("collapse");
      }
    },
    true
  );

  function build() {
    if (!document.body || document.getElementById(ID)) return;

    var root = document.createElement("div");
    root.id = ID;

    var circle = document.createElement("div");
    circle.className = "sk-circle";
    circle.setAttribute("data-tauri-drag-region", "");
    circle.setAttribute("aria-label", "Messenger");
    circle.innerHTML = LOGO;

    var badge = document.createElement("div");
    badge.id = ID + "_badge";
    circle.appendChild(badge);

    circle.addEventListener("click", function (e) {
      e.preventDefault();
      e.stopPropagation();
      act("expand");
    });
    circle.addEventListener("contextmenu", function (e) {
      e.preventDefault();
      e.stopPropagation();
      act("menu");
    });

    root.appendChild(circle);
    document.body.appendChild(root);
    apply(window.__skMode);
  }

  build();
  document.addEventListener("DOMContentLoaded", build);
  // Facebook is an SPA and can wipe the DOM; re-add the overlay and refresh the
  // unread badge on an interval.
  setInterval(function () {
    build();
    if (window.__skMode === "bubble") {
      setPageHidden(true);
      updateBadge();
    }
  }, 1500);
})();
