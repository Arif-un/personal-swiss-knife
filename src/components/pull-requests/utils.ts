import { DEFAULT_LIMIT, type Filters, type PrCheck, type PullRequest } from "./types.ts";

/** Label whose presence marks a PR for CI; compared case-insensitively. */
export const CI_LABEL = "CI";

/** Count how many filters differ from their default (for the filter badge). */
export function countActive(f: Filters): number {
  let n = 0;
  if (f.state !== "open") n++;
  if (f.author.trim()) n++;
  if (f.assignee.trim()) n++;
  if (f.labels.trim()) n++;
  if (f.base.trim()) n++;
  if (f.head.trim()) n++;
  if (f.search.trim()) n++;
  if (f.draftOnly) n++;
  if (f.limit !== DEFAULT_LIMIT) n++;
  return n;
}

/** Colored dot + label reflecting a PR's overall status. */
export function statusDot(pr: PullRequest, queued: boolean): { color: string; label: string } {
  if (pr.state.toUpperCase() === "MERGED") return { color: "bg-purple-500", label: "Merged" };
  if (pr.isDraft) return { color: "bg-muted-foreground/40", label: "Draft" };
  if (queued) return { color: "bg-orange-500", label: "In merge queue" };
  if (pr.state.toUpperCase() === "CLOSED") return { color: "bg-red-500", label: "Closed" };
  return { color: "bg-blue-500", label: "Open" };
}

export function hasCiLabel(pr: PullRequest): boolean {
  return pr.labels.some((l) => l.toLowerCase() === CI_LABEL.toLowerCase());
}

export function formatDate(dateStr: string): string {
  return new Date(dateStr).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

/** Group checks by workflow, preserving first-seen order of both. */
export function groupChecksByWorkflow(
  checks: PrCheck[],
): { workflow: string; checks: PrCheck[] }[] {
  const groups: { workflow: string; checks: PrCheck[] }[] = [];
  for (const c of checks) {
    const key = c.workflow || "Other";
    let group = groups.find((g) => g.workflow === key);
    if (!group) {
      group = { workflow: key, checks: [] };
      groups.push(group);
    }
    group.checks.push(c);
  }
  return groups;
}
