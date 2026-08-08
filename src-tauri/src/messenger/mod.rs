pub mod commands;

/// Window label for the warm, long-lived Messenger webview.
pub const MESSENGER_LABEL: &str = "messenger";
/// Window label for the reusable, disposable link-preview webview.
pub const PEEK_LABEL: &str = "peek";
/// messenger.com is deprecated; the messages surface lives under facebook.com.
pub const MESSENGER_URL: &str = "https://www.facebook.com/messages/";

/// Custom URL scheme the injected click interceptor navigates to in order to
/// signal link-routing intent to the Rust `on_navigation` handler. Using a
/// scheme (instead of `invoke`) means the Messenger page — remote, untrusted web
/// content — never gets ACL access to the app's real commands.
pub const ROUTE_SCHEME: &str = "swissknife-link";
