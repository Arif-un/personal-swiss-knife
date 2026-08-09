pub mod bubble;
pub mod commands;

/// Window label for the warm, long-lived Messenger webview.
pub const MESSENGER_LABEL: &str = "messenger";
/// Window label for the reusable, disposable link-preview webview.
pub const PEEK_LABEL: &str = "peek";
/// messenger.com is deprecated; the messages surface lives under facebook.com.
pub const MESSENGER_URL: &str = "https://www.facebook.com/messages/";

/// Custom URL scheme the injected click interceptor navigates to in order to
/// signal link-routing intent to the Rust `on_navigation` handler. The overlay
/// uses this scheme (not `invoke`) for its own actions so it never depends on the
/// IPC bridge. Note: app-defined commands are NOT gated by the capability ACL, so
/// the real defense against the remote, untrusted Messenger page invoking them is
/// the per-command caller check in `crate::security::require_main`.
pub const ROUTE_SCHEME: &str = "swissknife-link";
