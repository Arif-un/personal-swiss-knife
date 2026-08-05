/** Default SSH port; mirrors the backend `DEFAULT_SSH_PORT`. */
export const DEFAULT_SSH_PORT = 22;

/** Default local bind address for a new port forward. */
export const DEFAULT_BIND_ADDR = "127.0.0.1";

/** Terminal background; kept in one place so the xterm theme and the tab
 *  container that frames it can never drift apart. */
export const TERM_BACKGROUND = "#181825";

export const TERM_THEME = {
  background: TERM_BACKGROUND,
  foreground: "#cdd6f4",
  cursor: "#cdd6f4",
  selectionBackground: "#414458",
};

/** How long a toast stays visible, in ms. */
export const TOAST_DURATION_MS = 2000;
