import { useQuery } from "@tanstack/react-query";
import {
  Ban,
  CheckCircle2,
  Clock,
  Loader2,
  MinusCircle,
  XCircle,
} from "lucide-react";
import { prApi, prKeys } from "./api.ts";
import type { PrCheck } from "./types.ts";
import { groupChecksByWorkflow } from "./utils.ts";

// Icon reflecting a single check's outcome, keyed by `gh pr checks` bucket.
function checkIcon(bucket: string) {
  switch (bucket) {
    case "pass":
      return <CheckCircle2 className="size-3 shrink-0 text-green-500" />;
    case "fail":
      return <XCircle className="size-3 shrink-0 text-red-500" />;
    case "pending":
      return <Clock className="size-3 shrink-0 text-amber-500" />;
    case "cancel":
      return <Ban className="size-3 shrink-0 text-muted-foreground/50" />;
    case "skipping":
    default:
      return <MinusCircle className="size-3 shrink-0 text-muted-foreground/50" />;
  }
}

/** Lazily fetched CI checks for one PR, grouped by workflow. Mounted only while
 *  the row is expanded; a fresh fetch runs on every expand (no cache reuse). */
export function PrCheckList({ repo, number }: { repo: string; number: number }) {
  const { data, isLoading, error } = useQuery<PrCheck[]>({
    queryKey: prKeys.checks(repo, number),
    queryFn: () => prApi.checks(repo, number),
    enabled: repo.trim().length > 0,
    staleTime: 0,
    gcTime: 0,
    refetchOnMount: "always",
  });

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 p-3 text-[8px] text-muted-foreground">
        <Loader2 className="size-3 animate-spin" />
        Loading checks…
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-3 text-[8px] text-destructive">
        {error instanceof Error ? error.message : "Failed to load checks"}
      </div>
    );
  }

  if (!data || data.length === 0) {
    return (
      <div className="p-3 text-[8px] text-muted-foreground">
        No checks reported.
      </div>
    );
  }

  const groups = groupChecksByWorkflow(data);

  return (
    <div className="flex flex-col gap-3 p-3">
      {groups.map((group) => (
        <div key={group.workflow} className="flex flex-col gap-1">
          <div className="text-[8px] font-semibold uppercase tracking-wide text-muted-foreground/70">
            {group.workflow}
          </div>
          <div className="flex flex-wrap items-center gap-x-4 gap-y-1">
            {group.checks.map((c, i) => (
              <div
                key={`${c.name}-${i}`}
                className="flex items-center gap-1.5 text-[8px]"
              >
                {checkIcon(c.bucket)}
                {c.link ? (
                  <a
                    href={c.link}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="hover:text-foreground hover:underline"
                  >
                    {c.name}
                  </a>
                ) : (
                  <span>{c.name}</span>
                )}
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
