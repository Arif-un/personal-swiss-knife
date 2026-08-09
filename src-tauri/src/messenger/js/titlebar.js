(function () {
  var ID = "__sk_titlebar";
  function act(a) { window.location.href = "swissknife-link://window?action=" + a; }
  function makeDot(color, glyph, label, action) {
    var b = document.createElement("button");
    b.type = "button";
    b.setAttribute("aria-label", label);
    b.dataset.glyph = glyph;
    b.style.cssText = [
      "all:unset", "box-sizing:border-box", "width:12px", "height:12px",
      "border-radius:50%", "background:" + color, "cursor:default",
      "display:inline-flex", "align-items:center", "justify-content:center",
      "font-size:9px", "font-weight:700", "line-height:1",
      "color:rgba(0,0,0,0.55)", "pointer-events:auto",
      "font-family:-apple-system,BlinkMacSystemFont,system-ui,sans-serif"
    ].join(";");
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
    bar.style.cssText = [
      "position:fixed", "top:0px", "left:0px", "z-index:2147483647",
      "display:flex", "align-items:center", "gap:8px", "height:20px",
      "padding:0 10px", "border-radius:11px",
      "background:rgba(28,28,30,0.55)",
      "-webkit-backdrop-filter:blur(14px)", "backdrop-filter:blur(14px)",
      "box-shadow:0 1px 6px rgba(0,0,0,0.3)",
      "-webkit-user-select:none", "user-select:none", "pointer-events:auto"
    ].join(";");
    var dots = [
      makeDot("#ff5f57", "×", "Close", "close"),
      makeDot("#febc2e", "–", "Minimize", "minimize"),
      makeDot("#28c840", "+", "Zoom", "zoom")
    ];
    dots.forEach(function (d) { bar.appendChild(d); });
    bar.addEventListener("mouseenter", function () {
      dots.forEach(function (d) { d.textContent = d.dataset.glyph; });
    });
    bar.addEventListener("mouseleave", function () {
      dots.forEach(function (d) { d.textContent = ""; });
    });
    document.body.appendChild(bar);

    // Slim, full-width drag strip along the top edge. Thin at rest so it barely
    // covers the page; grows + shows a grip on hover to invite dragging. Sits
    // just below the control pill so the pill's buttons stay clickable.
    var strip = document.createElement("div");
    strip.id = ID + "_drag";
    strip.setAttribute("data-tauri-drag-region", "");
    strip.style.cssText = [
      "position:fixed", "top:0", "left:0", "right:0", "height:6px",
      "z-index:2147483646", "pointer-events:auto", "background:transparent", "backdrop-filter:blur(6px)",
      "-webkit-user-select:none", "user-select:none",
      "transition:height .15s ease, background .15s ease"
    ].join(";");
    var grip = document.createElement("div");
    grip.style.cssText = [
      "position:absolute", "top:7px", "left:50%", "transform:translateX(-50%)",
      "width:46px", "height:5px", "border-radius:3px",
      "background:rgba(255,255,255,0.4)", "opacity:0",
      "transition:opacity .15s ease", "pointer-events:none"
    ].join(";");
    strip.appendChild(grip);
    strip.addEventListener("mouseenter", function () {
      strip.style.height = "22px";
      strip.style.background = "rgba(28,28,30,0.35)";
      grip.style.opacity = "1";
    });
    strip.addEventListener("mouseleave", function () {
      strip.style.height = "6px";
      strip.style.background = "transparent";
      grip.style.opacity = "0";
    });
    document.body.appendChild(strip);
  }
  build();
  document.addEventListener("DOMContentLoaded", build);
  setInterval(build, 1500);
})();
