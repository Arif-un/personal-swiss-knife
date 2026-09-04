import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import * as React from "react";

/** Runtime look-and-feel, backed by `branding.json` (Rust `settings` module). */
export interface Branding {
  displayName: string;
  /** Any CSS colour string; applied verbatim to the accent CSS variables. */
  accentColor: string;
}

export const BRANDING_DEFAULTS: Branding = {
  displayName: "Swiss Knife",
  accentColor: "oklch(0.488 0.243 264.376)",
};

type BrandingContextProps = {
  branding: Branding;
  setBranding: (b: Branding) => Promise<void>;
};

const BrandingContext = React.createContext<BrandingContextProps | null>(null);

/** Push branding into the DOM + native window: accent onto the CSS variables the
 * theme reads, and the display name onto the document + window title. */
function applyBranding(b: Branding) {
  const root = document.documentElement.style;
  for (const v of ["--primary", "--sidebar-primary", "--ring"]) {
    root.setProperty(v, b.accentColor);
  }
  document.title = b.displayName;
  void getCurrentWindow().setTitle(b.displayName);
}

export function BrandingProvider({ children }: { children: React.ReactNode }) {
  const [branding, setState] = React.useState<Branding>(BRANDING_DEFAULTS);

  React.useEffect(() => {
    invoke<Branding>("branding_get")
      .then((b) => {
        setState(b);
        applyBranding(b);
      })
      .catch(() => applyBranding(BRANDING_DEFAULTS));
  }, []);

  const setBranding = React.useCallback(async (b: Branding) => {
    await invoke("branding_set", { branding: b });
    setState(b);
    applyBranding(b);
  }, []);

  const value = React.useMemo(() => ({ branding, setBranding }), [branding, setBranding]);

  return <BrandingContext.Provider value={value}>{children}</BrandingContext.Provider>;
}

export function useBranding() {
  const ctx = React.useContext(BrandingContext);
  if (!ctx) throw new Error("useBranding must be used within BrandingProvider");
  return ctx;
}
