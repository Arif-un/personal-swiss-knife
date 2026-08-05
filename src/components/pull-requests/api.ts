import { invoke } from "@tauri-apps/api/core";
import type { Filters, PrCheck, PrView, PrViewsStore, PullRequest } from "./types.ts";

/** All PR-feature IPC in one place (parity with the SSH feature's `sshApi`). */
export const prApi = {
  list: (repo: string, filters: Filters) =>
    invoke<PullRequest[]>("fetch_pull_requests", { repo, filters }),
  ciLabelCounts: (repo: string, numbers: number[]) =>
    invoke<Record<string, number>>("fetch_ci_label_counts", { repo, numbers }),
  unresolvedCounts: (repo: string, numbers: number[]) =>
    invoke<Record<string, number>>("fetch_unresolved_comment_counts", {
      repo,
      numbers,
    }),
  mergeQueueStatus: (repo: string, numbers: number[]) =>
    invoke<Record<string, boolean>>("fetch_merge_queue_status", {
      repo,
      numbers,
    }),
  readdCiLabel: (repo: string, number: number) =>
    invoke<string[]>("readd_ci_label", { repo, number }),
  checks: (repo: string, number: number) => invoke<PrCheck[]>("fetch_pr_checks", { repo, number }),
};

export const prViewsApi = {
  list: () => invoke<PrViewsStore>("pr_views_list"),
  save: (view: Omit<PrView, "id"> & { id?: string }) => invoke<PrView>("pr_views_save", { view }),
  delete: (id: string) => invoke<void>("pr_views_delete", { id }),
  setActive: (id: string | null) => invoke<void>("pr_views_set_active", { id }),
};

/** Central react-query key registry — avoids scattered string literals. */
export const prKeys = {
  list: (repo: string, filters: Filters) => ["pull-requests", repo, filters] as const,
  ciCounts: (repo: string, numbers: number[]) => ["ci-label-counts", repo, numbers] as const,
  ciCountsRoot: (repo: string) => ["ci-label-counts", repo] as const,
  unresolved: (repo: string, numbers: number[]) =>
    ["unresolved-comment-counts", repo, numbers] as const,
  mergeQueue: (repo: string, numbers: number[]) => ["merge-queue-status", repo, numbers] as const,
  checks: (repo: string, number: number) => ["pr-checks", repo, number] as const,
  views: () => ["pr-views"] as const,
};
