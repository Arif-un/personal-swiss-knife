// Linux Tauri renders via WebKitGTK, where several macOS-only window features
// are no-ops: the overlay title bar, hidden title, vibrancy `windowEffects`,
// and traffic-light buttons. Layout that reserves space for those (e.g. the
// `pl-21` traffic-light gutter) must be dropped on Linux, and `backdrop-filter`
// must be avoided there because it mis-composites. Detect once at load.
export const isLinux = /linux/i.test(navigator.userAgent);
