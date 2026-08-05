import { useCallback } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** onMouseDown handler that starts a native window drag, except when the press
 *  lands on an interactive element (button / link / input). */
export function useWindowDrag() {
  return useCallback((e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button, a, input")) return;
    getCurrentWindow().startDragging();
  }, []);
}
