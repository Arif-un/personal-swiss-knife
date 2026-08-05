import { Fragment, memo } from "react";
import { ChevronRight, FlaskConical, Loader2, MessageSquare } from "lucide-react";
import { Badge } from "#components/ui/badge.tsx";
import { Button } from "#components/ui/button.tsx";
import { TableCell, TableRow } from "#components/ui/table.tsx";
import { PrCheckList } from "./PrCheckList.tsx";
import type { PullRequest } from "./types.ts";
import { CI_LABEL, formatDate, hasCiLabel, statusDot } from "./utils.ts";

interface PrRowProps {
  pr: PullRequest;
  repo: string;
  isExpanded: boolean;
  queued: boolean;
  ciCount: number;
  unresolvedCount: number;
  isCiPending: boolean;
  onToggle: (number: number) => void;
  onCiMutate: (number: number) => void;
}

function PrRowImpl({
  pr,
  repo,
  isExpanded,
  queued,
  ciCount,
  unresolvedCount,
  isCiPending,
  onToggle,
  onCiMutate,
}: PrRowProps) {
  const hasCi = hasCiLabel(pr);
  const dot = statusDot(pr, queued);

  return (
    <Fragment>
      <TableRow
        aria-expanded={isExpanded}
        className="cursor-pointer"
        onClick={(e) => {
          // Let links/buttons in the row keep their own behavior.
          if ((e.target as HTMLElement).closest("a,button")) return;
          onToggle(pr.number);
        }}
      >
        <TableCell className="font-mono text-muted-foreground">
          <span className="inline-flex items-center gap-1">
            <ChevronRight
              className={`size-3 shrink-0 text-muted-foreground/60 transition-transform ${isExpanded ? "rotate-90" : ""}`}
            />
            {pr.number}
          </span>
        </TableCell>
        <TableCell>
          <span className="inline-flex items-center gap-1.5">
            <span
              className={`inline-block size-2 shrink-0 rounded-full ${dot.color}`}
              title={dot.label}
            />
            <a
              href={pr.url}
              target="_blank"
              rel="noopener noreferrer"
              className="font-medium hover:underline"
            >
              {pr.title}
            </a>
          </span>
          <div
            className={
              unresolvedCount > 0
                ? "mt-0.5 flex w-fit items-center gap-1 text-[9px] text-amber-600 dark:text-amber-500"
                : "mt-0.5 flex w-fit items-center gap-1 text-[9px] text-muted-foreground/40"
            }
            title={
              unresolvedCount > 0
                ? `${unresolvedCount} unresolved comment${unresolvedCount !== 1 ? "s" : ""}`
                : "No unresolved comments"
            }
          >
            <MessageSquare className="size-3" />
            {unresolvedCount > 0 && (
              <span className="tabular-nums">{unresolvedCount}</span>
            )}
          </div>
        </TableCell>
        <TableCell className="text-muted-foreground">{pr.author}</TableCell>
        <TableCell>
          <code className="rounded bg-muted px-1.5 py-0.5 text-[10px]">
            {pr.headRefName}
          </code>
        </TableCell>
        <TableCell className="text-muted-foreground">
          {formatDate(pr.createdAt)}
        </TableCell>
        <TableCell className="text-center">
          <div className="relative inline-flex">
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={() => onCiMutate(pr.number)}
              disabled={isCiPending}
              aria-pressed={hasCi}
              title={
                hasCi
                  ? `Re-add ${CI_LABEL} label (added ${ciCount}× so far)`
                  : `Add ${CI_LABEL} label${ciCount ? ` (added ${ciCount}× so far)` : ""}`
              }
              className={
                hasCi ? "text-blue-500" : "opacity-40 hover:opacity-100"
              }
            >
              {isCiPending ? (
                <Loader2 className="animate-spin" />
              ) : (
                <FlaskConical />
              )}
            </Button>
            {ciCount > 0 && (
              <Badge
                variant="secondary"
                className="pointer-events-none absolute -right-1 -top-1 h-4 min-w-4 justify-center rounded-full px-1 text-[9px] leading-none tabular-nums"
              >
                {ciCount}
              </Badge>
            )}
          </div>
        </TableCell>
      </TableRow>
      {isExpanded && (
        <TableRow>
          <TableCell colSpan={6} className="bg-muted/30 p-0">
            <PrCheckList repo={repo} number={pr.number} />
          </TableCell>
        </TableRow>
      )}
    </Fragment>
  );
}

export const PrRow = memo(PrRowImpl);
