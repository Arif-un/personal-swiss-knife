// Injects the Messenger overlay stylesheet (css/messenger.css, handed in via the
// window.__SK_CSS global that Rust sets before this runs) as a <style> element.
// Re-appended on an interval because Facebook is an SPA that can wipe the DOM.
(function () {
  var ID = "__sk_style";
  function inject() {
    var d = document;
    var head = d.head || d.documentElement;
    if (!head || d.getElementById(ID) || !window.__SK_CSS) return;
    var s = d.createElement("style");
    s.id = ID;
    s.textContent = window.__SK_CSS;
    head.appendChild(s);
  }
  inject();
  document.addEventListener("DOMContentLoaded", inject);
  setInterval(inject, 1500);
})();
