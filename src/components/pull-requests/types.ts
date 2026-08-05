/** Default number of PRs to list; mirrors the backend `DEFAULT_LIMIT`. */
export const DEFAULT_LIMIT = 30;

/** Filter `state` values offered by the UI (maps to `gh pr list --state`). */
export type PrState = "open" | "closed" | "merged" | "all";

export interface Filters {
  state: PrState;
  author: string;
  assignee: string;
  labels: string;
  base: string;
  head: string;
  search: string;
  draftOnly: boolean;
  limit: number;
}

/** A fresh, unfiltered filter set. A factory (not a shared const) so callers
 *  can never accidentally mutate a shared object. */
export function makeEmptyFilters(): Filters {
  return {
    state: "open",
    author: "",
    assignee: "",
    labels: "",
    base: "",
    head: "",
    search: "",
    draftOnly: false,
    limit: DEFAULT_LIMIT,
  };
}

export interface PullRequest {
  number: number;
  title: string;
  author: string;
  url: string;
  createdAt: string;
  headRefName: string;
  isDraft: boolean;
  /** Raw gh state, e.g. OPEN / CLOSED / MERGED (compared case-insensitively). */
  state: string;
  labels: string[];
}

/** `gh pr checks` outcome buckets we render distinctly; other values fall back
 *  to the "skipping" icon. */
export type CheckBucket = "pass" | "fail" | "pending" | "cancel" | "skipping";

export interface PrCheck {
  name: string;
  workflow: string;
  bucket: string;
  state: string;
  link: string;
  startedAt: string;
  completedAt: string;
}

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
