export interface Filters {
  state: string;
  author: string;
  assignee: string;
  labels: string;
  base: string;
  head: string;
  search: string;
  draftOnly: boolean;
  limit: number;
}

export const emptyFilters: Filters = {
  state: "open",
  author: "",
  assignee: "",
  labels: "",
  base: "",
  head: "",
  search: "",
  draftOnly: false,
  limit: 30,
};

/** A saved repo + filter preset, persisted by the backend. */
export interface PrView {
  id: string;
  name: string;
  repo: string;
  filters: Filters;
}

export interface PrViewsStore {
  views: PrView[];
  activeViewId: string | null;
}
