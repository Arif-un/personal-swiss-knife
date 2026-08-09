(function () {
  var INTERNAL = /(^|\.)facebook\.com$|(^|\.)fbcdn\.net$|(^|\.)messenger\.com$|(^|\.)fb\.com$/;
  function unwrap(raw) {
    try {
      var u = new URL(raw, location.href);
      if (u.hostname === "l.facebook.com" || u.hostname === "lm.facebook.com") {
        var t = u.searchParams.get("u");
        if (t) return t;
      }
      return u.href;
    } catch (e) {
      return raw;
    }
  }
  function isInternal(raw) {
    try { return INTERNAL.test(new URL(raw, location.href).hostname); }
    catch (e) { return true; }
  }
  // Returns true if the link was captured (caller should cancel the default).
  function route(raw, toBrowser) {
    var target = unwrap(raw);
    if (isInternal(target)) return false;
    var mode = toBrowser ? "browser" : "peek";
    location.href = "swissknife-link://route?mode=" + mode + "&url=" + encodeURIComponent(target);
    return true;
  }
  document.addEventListener("click", function (e) {
    if (e.defaultPrevented || e.button !== 0) return;
    var a = e.target && e.target.closest ? e.target.closest("a[href]") : null;
    if (!a) return;
    if (route(a.href, e.shiftKey)) {
      e.preventDefault();
      e.stopPropagation();
    }
  }, true);
})();
