import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "#components/ui/table.tsx";
import { PrRow } from "./PrRow.tsx";
import type { PullRequest } from "./types.ts";

interface PrTableProps {
  prs: PullRequest[];
  repo: string;
  expanded: Set<number>;
  ciCounts: Record<string, number>;
  unresolvedCounts: Record<string, number>;
  mergeQueueStatus: Record<string, boolean>;
  ciPendingNumber: number | null;
  onToggle: (number: number) => void;
  onCiMutate: (number: number) => void;
}

export function PrTable({
  prs,
  repo,
  expanded,
  ciCounts,
  unresolvedCounts,
  mergeQueueStatus,
  ciPendingNumber,
  onToggle,
  onCiMutate,
}: PrTableProps) {
  return (
    <div className="rounded-md border">
      <Table className="text-[10px]">
        <TableHeader>
          <TableRow>
            <TableHead className="w-16">#</TableHead>
            <TableHead>Title</TableHead>
            <TableHead className="w-32">Author</TableHead>
            <TableHead className="w-28">Date</TableHead>
            <TableHead className="w-12 text-center">CI</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {prs.length === 0 && (
            <TableRow>
              <TableCell colSpan={5} className="text-center text-muted-foreground">
                No pull requests match these filters.
              </TableCell>
            </TableRow>
          )}
          {prs.map((pr) => {
            const key = String(pr.number);
            return (
              <PrRow
                key={pr.number}
                pr={pr}
                repo={repo}
                isExpanded={expanded.has(pr.number)}
                queued={mergeQueueStatus[key] ?? false}
                ciCount={ciCounts[key] ?? 0}
                unresolvedCount={unresolvedCounts[key] ?? 0}
                isCiPending={ciPendingNumber === pr.number}
                onToggle={onToggle}
                onCiMutate={onCiMutate}
              />
            );
          })}
        </TableBody>
      </Table>
    </div>
  );
}
