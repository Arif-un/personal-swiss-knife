(function () {
  var INTERNAL = /(^|\.)facebook\.com$|(^|\.)fbcdn\.net$|(^|\.)messenger\.com$|(^|\.)fb\.com$/;
  // Unwrap Facebook's l.facebook.com / lm.facebook.com link-shim to the real
  // destination so classification and routing use the actual target URL.
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
  // Messenger's own app lives under /messages (thread list, open threads, the
  // rail). Those in-app routes must stay with the SPA, so we never capture them.
  function isMessengerAppRoute(path) {
    return /^\/messages(\/|$)/.test(path);
  }
  // Report a click to Rust, which applies the user's routing rules
  // (on_navigation). Returns true if captured, so the caller cancels the
  // default navigation.
  function route(a, e) {
    var target = unwrap(a.href);
    var fb = false;
    var path = "";
    try {
      var u = new URL(target, location.href);
      fb = INTERNAL.test(u.hostname);
      path = u.pathname;
    } catch (err) {
      // Unparseable/relative: it resolves against facebook.com, so treat as
      // internal app navigation and leave it alone.
      return false;
    }
    // Leave Messenger's own navigation (switching chats, the rail) to the SPA;
    // our capture-phase handler would otherwise cancel the in-app route. Every
    // other Facebook link (posts, reels, profiles, marketplace) is shared
    // content and gets captured; non-Facebook links are never app nav.
    if (fb && isMessengerAppRoute(path)) return false;
    location.href =
      "swissknife-link://route?fb=" + (fb ? 1 : 0) +
      "&meta=" + (e.metaKey ? 1 : 0) +
      "&ctrl=" + (e.ctrlKey ? 1 : 0) +
      "&alt=" + (e.altKey ? 1 : 0) +
      "&shift=" + (e.shiftKey ? 1 : 0) +
      "&url=" + encodeURIComponent(target);
    return true;
  }
  document.addEventListener("click", function (e) {
    if (e.defaultPrevented || e.button !== 0) return;
    var a = e.target && e.target.closest ? e.target.closest("a[href]") : null;
    if (!a) return;
    if (route(a, e)) {
      e.preventDefault();
      e.stopPropagation();
    }
  }, true);
})();
