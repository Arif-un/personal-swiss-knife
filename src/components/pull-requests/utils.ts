import { CheckCircle2, Clock, XCircle, type LucideIcon } from "lucide-react";
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

/** Icon + color + label reflecting a PR's review approval status. */
export function reviewStatus(pr: PullRequest): {
  icon: LucideIcon;
  color: string;
  label: string;
} {
  switch (pr.reviewDecision?.toUpperCase()) {
    case "APPROVED":
      return { icon: CheckCircle2, color: "text-green-500", label: "Approved" };
    case "CHANGES_REQUESTED":
      return { icon: XCircle, color: "text-red-500", label: "Changes requested" };
    default:
      return { icon: Clock, color: "text-amber-500", label: "Review pending" };
  }
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

/** Compact relative time, e.g. "3h ago", "2d ago", "5mo ago". */
export function timeAgo(dateStr: string): string {
  const secs = Math.round((Date.now() - new Date(dateStr).getTime()) / 1000);
  if (secs < 60) return "just now";
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  const days = Math.floor(hrs / 24);
  if (days < 30) return `${days}d ago`;
  const mos = Math.floor(days / 30);
  if (mos < 12) return `${mos}mo ago`;
  return `${Math.floor(mos / 12)}y ago`;
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
