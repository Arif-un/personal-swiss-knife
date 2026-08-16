//! Link-routing rules for clicks inside the Messenger webview.

/// Where a clicked link is opened. Serialized to the settings page (and disk) in
/// kebab-case (`same-window`, `child-webview`, `system-browser`).
#[derive(Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LinkAction {
    /// Navigate the main Messenger webview to the link, replacing the current view.
    SameWindow,
    /// Open the link in the in-window preview child webview (a modal).
    ChildWebview,
    /// Hand the link to the user's default system browser.
    SystemBrowser,
}

/// A modifier-combo override: when exactly these modifiers are held on the click,
/// use `action` regardless of the destination default. An override with no
/// modifier set never fires (that would swallow every plain click).
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct LinkOverride {
    #[serde(default)]
    pub meta: bool,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub shift: bool,
    pub action: LinkAction,
}

impl LinkOverride {
    fn matches(&self, meta: bool, ctrl: bool, alt: bool, shift: bool) -> bool {
        (self.meta || self.ctrl || self.alt || self.shift)
            && self.meta == meta
            && self.ctrl == ctrl
            && self.alt == alt
            && self.shift == shift
    }
}

/// User-editable link-routing config (Messenger settings page). Destination
/// defaults route by whether the (shim-unwrapped) URL is a Facebook-family host;
/// `overrides` are modifier combos that win over the default when matched.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct LinkRules {
    pub facebook: LinkAction,
    pub other: LinkAction,
    pub overrides: Vec<LinkOverride>,
}

impl Default for LinkRules {
    fn default() -> Self {
        Self {
            facebook: LinkAction::ChildWebview,
            other: LinkAction::SystemBrowser,
            // Cmd+Shift+click -> load in the same window.
            overrides: vec![LinkOverride {
                meta: true,
                ctrl: false,
                alt: false,
                shift: true,
                action: LinkAction::SameWindow,
            }],
        }
    }
}

impl LinkRules {
    /// Resolve the action for a click: the first matching modifier override wins
    /// (modifiers are an explicit user intent), else the per-destination default.
    pub fn resolve(
        &self,
        is_fb: bool,
        meta: bool,
        ctrl: bool,
        alt: bool,
        shift: bool,
    ) -> LinkAction {
        if let Some(o) = self
            .overrides
            .iter()
            .find(|o| o.matches(meta, ctrl, alt, shift))
        {
            return o.action;
        }
        if is_fb {
            self.facebook
        } else {
            self.other
        }
    }
}
