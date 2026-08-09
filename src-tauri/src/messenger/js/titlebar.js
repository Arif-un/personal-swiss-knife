(function () {
  var ID = "__sk_titlebar";
  window.__skMode = window.__skMode || "full";
  function act(a) { window.location.href = "swissknife-link://window?action=" + a; }
  // Hide the control pill + drag strip while collapsed to a bubble (rules in
  // messenger.css).
  function applyMode() {
    var bubble = window.__skMode === "bubble";
    var bar = document.getElementById(ID);
    var strip = document.getElementById(ID + "_drag");
    if (bar) bar.classList.toggle("sk-hidden", bubble);
    if (strip) strip.classList.toggle("sk-hidden", bubble);
  }
  window.addEventListener("__sk:mode", applyMode);
  function makeDot(glyph, label, action) {
    var b = document.createElement("button");
    b.type = "button";
    b.className = "sk-dot sk-dot-" + action;
    b.setAttribute("aria-label", label);
    // The glyph is revealed on pill hover via CSS (::after content:attr(data-glyph)).
    b.dataset.glyph = glyph;
    b.addEventListener("mousedown", function (e) { e.stopPropagation(); });
    b.addEventListener("click", function (e) {
      e.preventDefault(); e.stopPropagation(); act(action);
    });
    return b;
  }
  function build() {
    if (!document.body || document.getElementById(ID)) return;
    var bar = document.createElement("div");
    bar.id = ID;
    bar.setAttribute("data-tauri-drag-region", "");
    var dots = [
      makeDot("×", "Close", "close"),
      makeDot("–", "Minimize", "minimize"),
      makeDot("+", "Zoom", "zoom"),
      makeDot("⇲", "Collapse to bubble", "collapse")
    ];
    dots.forEach(function (d) { bar.appendChild(d); });
    document.body.appendChild(bar);

    // Slim, full-width drag strip along the top edge. Thin at rest so it barely
    // covers the page; grows + shows a grip on hover to invite dragging (CSS
    // :hover). Sits just below the control pill so the pill's buttons stay
    // clickable.
    var strip = document.createElement("div");
    strip.id = ID + "_drag";
    strip.setAttribute("data-tauri-drag-region", "");
    // Double-click zooms (maximize / restore old size), like a native title bar.
    // The 2nd mousedown is stopped from reaching Tauri's own drag-region
    // double-click maximize so the two handlers don't toggle each other out.
    strip.addEventListener("mousedown", function (e) {
      if (e.detail === 2) e.stopPropagation();
    });
    strip.addEventListener("dblclick", function (e) {
      e.preventDefault();
      e.stopPropagation();
      act("zoom");
    });
    var grip = document.createElement("div");
    grip.className = "sk-grip";
    strip.appendChild(grip);
    document.body.appendChild(strip);
    applyMode();
  }
  build();
  document.addEventListener("DOMContentLoaded", build);
  setInterval(build, 1500);
})();
