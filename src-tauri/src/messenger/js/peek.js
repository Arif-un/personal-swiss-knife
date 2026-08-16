// Modal "peek" frame for the link-preview panel (see commands.rs::open_peek).
// The preview content itself is a NATIVE child webview that always paints above
// this page, so this only draws the dim backdrop and the header bar that sit
// AROUND it: the child webview rect is inset below the header.
//
// This page is ALSO the source of truth for the child webview's geometry. Rust
// can't place the native child in the page's CSS pixels directly (its
// scale_factor / inner_size are logical px, which diverge from CSS px under
// HiDPI scaled display modes -> FB spills past the frame). So we measure the
// hole here in CSS px and report it via `swissknife-link://peek?x&y&w&h`; Rust
// sizes the child to exactly that. MARGIN/HEADER must match messenger.css.
// Close routes through the shared swissknife-link:// scheme, the same channel
// the title bar uses, so the remote page still gets no IPC access.
(function () {
  var ID = "__sk_peek";
  var MARGIN = 24;
  var HEADER = 40;
  var active = false;
  function close() {
    window.location.href = "swissknife-link://window?action=peek-close";
  }
  // Report the modal hole (in CSS px) so Rust can size the child webview to it.
  function reportRect() {
    if (!active) return;
    var w = window.innerWidth;
    var h = window.innerHeight;
    var y = MARGIN + HEADER;
    var cw = Math.max(120, w - 2 * MARGIN);
    var ch = Math.max(120, h - y - MARGIN);
    window.location.href =
      "swissknife-link://peek?x=" + MARGIN + "&y=" + y + "&w=" + cw + "&h=" + ch;
  }
  var pending = false;
  function scheduleReport() {
    if (pending || !active) return;
    pending = true;
    requestAnimationFrame(function () {
      pending = false;
      reportRect();
    });
  }
  function build() {
    var existing = document.getElementById(ID);
    if (existing || !document.body) return existing;
    var back = document.createElement("div");
    back.id = ID;
    // Click on the dim margin (anywhere the child webview doesn't cover) closes.
    back.addEventListener("click", close);
    var bar = document.createElement("div");
    bar.id = "__sk_peek_bar";
    bar.addEventListener("click", function (e) { e.stopPropagation(); });
    var label = document.createElement("span");
    label.id = "__sk_peek_url";
    var x = document.createElement("button");
    x.type = "button";
    x.id = "__sk_peek_close";
    x.setAttribute("aria-label", "Close preview");
    x.textContent = "×";
    x.addEventListener("click", close);
    bar.appendChild(label);
    bar.appendChild(x);
    back.appendChild(bar);
    document.body.appendChild(back);
    return back;
  }
  window.__skPeekShow = function (url) {
    active = true;
    window.__skPeekActive = true;
    var back = build();
    if (!back) return;
    var label = document.getElementById("__sk_peek_url");
    if (label) label.textContent = url || "";
    back.classList.add("sk-on");
    reportRect();
  };
  window.__skPeekHide = function () {
    active = false;
    window.__skPeekActive = false;
    var back = document.getElementById(ID);
    if (back) back.classList.remove("sk-on");
  };
  // Keep the child webview matched to the window as it resizes.
  window.addEventListener("resize", scheduleReport);
  // Esc closes the preview instead of collapsing the window (bubble.js defers to
  // us via __skPeekActive). Capture phase so we win over Facebook's handlers.
  // This only fires while focus is in THIS page; once the user clicks into the
  // panel, focus moves to the native child webview, which has its own Esc
  // handler (see commands.rs::PEEK_ESC_JS).
  document.addEventListener(
    "keydown",
    function (e) {
      if (active && e.key === "Escape") {
        e.preventDefault();
        e.stopImmediatePropagation();
        close();
      }
    },
    true
  );
  // Facebook is an SPA that can wipe the DOM; re-add the frame while a preview is
  // open so it doesn't vanish under the (still-native) child webview.
  setInterval(function () {
    if (!active) return;
    var back = build();
    if (back) back.classList.add("sk-on");
  }, 1500);
})();
