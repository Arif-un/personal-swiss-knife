(function () {
  var ID = "__sk_titlebar";
  window.__skMode = window.__skMode || "full";
  function act(a) { window.location.href = "swissknife-link://window?action=" + a; }
  // Hide the control pill while collapsed to a bubble (rules in messenger.css).
  function applyMode() {
    var bubble = window.__skMode === "bubble";
    var bar = document.getElementById(ID);
    if (bar) bar.classList.toggle("sk-hidden", bubble);
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
  // Make Facebook's own nav bar a drag handle. Must be "deep": clicks land on
  // nested children, and a bare attr only drags on a direct hit of the tagged
  // node itself. "deep" drags anywhere in the subtree; Tauri's handler still
  // skips clickable children (a/button, role=link/button/tab...) so nav items
  // keep working. React re-renders / swaps the node, so re-tag every tick.
  function tagNav() {
    document.querySelectorAll('[role="navigation"]').forEach(function (n) {
      if (n.getAttribute("data-tauri-drag-region") !== "deep")
        n.setAttribute("data-tauri-drag-region", "deep");
    });
  }
  function build() {
    tagNav();
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
    applyMode();
  }
  build();
  document.addEventListener("DOMContentLoaded", build);
  setInterval(build, 1500);
})();
