import { invoke } from "@tauri-apps/api/core";
import type { PrView, PrViewsStore } from "./types.ts";

export const prViewsApi = {
  list: () => invoke<PrViewsStore>("pr_views_list"),
  save: (view: Omit<PrView, "id"> & { id?: string }) =>
    invoke<PrView>("pr_views_save", { view }),
  delete: (id: string) => invoke<void>("pr_views_delete", { id }),
  setActive: (id: string | null) => invoke<void>("pr_views_set_active", { id }),
};
