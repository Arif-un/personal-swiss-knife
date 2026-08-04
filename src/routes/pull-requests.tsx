import { useState } from "react";
import { createRoute } from "@tanstack/react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { FlaskConical, Loader2, SlidersHorizontal, X } from "lucide-react";
import { rootRoute } from "./__root.tsx";
import { Input } from "#components/ui/input.tsx";
import { Button } from "#components/ui/button.tsx";
import { Badge } from "#components/ui/badge.tsx";
import { Skeleton } from "#components/ui/skeleton.tsx";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "#components/ui/table.tsx";

interface PullRequest {
  number: number;
  title: string;
  author: string;
  url: string;
  createdAt: string;
  headRefName: string;
  isDraft: boolean;
  state: string;
  labels: string[];
}

const CI_LABEL = "CI";

interface Filters {
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

const emptyFilters: Filters = {
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

function countActive(f: Filters): number {
  let n = 0;
  if (f.state !== "open") n++;
  if (f.author.trim()) n++;
  if (f.assignee.trim()) n++;
  if (f.labels.trim()) n++;
  if (f.base.trim()) n++;
  if (f.head.trim()) n++;
  if (f.search.trim()) n++;
  if (f.draftOnly) n++;
  if (f.limit !== 30) n++;
  return n;
}

function stateBadge(pr: PullRequest): {
  label: string;
  variant: "default" | "secondary" | "destructive" | "outline";
} {
  if (pr.isDraft) return { label: "Draft", variant: "secondary" };
  switch (pr.state.toUpperCase()) {
    case "MERGED":
      return { label: "Merged", variant: "outline" };
    case "CLOSED":
      return { label: "Closed", variant: "destructive" };
    default:
      return { label: "Open", variant: "default" };
  }
}

function PullRequestsPage() {
  const [repo, setRepo] = useState("");
  const [filters, setFilters] = useState<Filters>(emptyFilters);
  const [showFilters, setShowFilters] = useState(false);

  // Committed values that actually drive the query.
  const [searchRepo, setSearchRepo] = useState("");
  const [appliedFilters, setAppliedFilters] = useState<Filters>(emptyFilters);

  const queryClient = useQueryClient();
  const prsQueryKey = ["pull-requests", searchRepo, appliedFilters];

  const {
    data: prs,
    isLoading,
    isFetching,
    error,
    refetch,
  } = useQuery<PullRequest[]>({
    queryKey: prsQueryKey,
    queryFn: () =>
      invoke<PullRequest[]>("fetch_pull_requests", {
        repo: searchRepo,
        filters: appliedFilters,
      }),
    enabled: searchRepo.trim().length > 0,
  });

  // Adds the CI label; if already present it is removed then re-added to force
  // a fresh label event. Backend returns the PR's labels after the operation.
  const ciMutation = useMutation({
    mutationFn: (number: number) =>
      invoke<string[]>("readd_ci_label", { repo: searchRepo, number }),
    onSuccess: (labels, number) => {
      queryClient.setQueryData<PullRequest[]>(prsQueryKey, (prev) =>
        prev?.map((pr) => (pr.number === number ? { ...pr, labels } : pr)),
      );
    },
  });

  function commit() {
    setSearchRepo(repo);
    setAppliedFilters(filters);
  }

  function handleSearch(e: React.FormEvent) {
    e.preventDefault();
    commit();
  }

  function resetFilters() {
    setFilters(emptyFilters);
    setAppliedFilters(emptyFilters);
    setSearchRepo(repo);
  }

  function setField<K extends keyof Filters>(key: K, value: Filters[K]) {
    setFilters((prev) => ({ ...prev, [key]: value }));
  }

  function formatDate(dateStr: string) {
    return new Date(dateStr).toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
    });
  }

  const activeCount = countActive(appliedFilters);

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <Button
          variant="outline"
          size="sm"
          onClick={() => refetch()}
          disabled={!searchRepo.trim() || isFetching}
        >
          Refresh
        </Button>
      </div>

      <div className="flex flex-col gap-3">
        <form onSubmit={handleSearch} className="flex gap-2">
          <Input
            value={repo}
            onChange={(e) => setRepo(e.target.value)}
            placeholder="owner/repo"
            className="max-w-sm"
          />
          <Button type="submit">Fetch</Button>
          <Button
            type="button"
            variant="outline"
            onClick={() => setShowFilters((s) => !s)}
            aria-expanded={showFilters}
          >
            <SlidersHorizontal />
            Filters
            {activeCount > 0 && (
              <Badge variant="secondary" className="ml-1">
                {activeCount}
              </Badge>
            )}
          </Button>
        </form>

        {showFilters && (
          <div className="rounded-md border p-4">
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
              <FilterField label="State">
                <select
                  value={filters.state}
                  onChange={(e) => setField("state", e.target.value)}
                  className="h-7 w-full rounded-md border border-input bg-transparent px-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 dark:bg-input/30"
                >
                  <option value="open">Open</option>
                  <option value="closed">Closed</option>
                  <option value="merged">Merged</option>
                  <option value="all">All</option>
                </select>
              </FilterField>

              <FilterField label="Author">
                <Input
                  value={filters.author}
                  onChange={(e) => setField("author", e.target.value)}
                  placeholder="@me or username"
                />
              </FilterField>

              <FilterField label="Assignee">
                <Input
                  value={filters.assignee}
                  onChange={(e) => setField("assignee", e.target.value)}
                  placeholder="@me or username"
                />
              </FilterField>

              <FilterField label="Labels">
                <Input
                  value={filters.labels}
                  onChange={(e) => setField("labels", e.target.value)}
                  placeholder="bug, enhancement"
                />
              </FilterField>

              <FilterField label="Base branch">
                <Input
                  value={filters.base}
                  onChange={(e) => setField("base", e.target.value)}
                  placeholder="main"
                />
              </FilterField>

              <FilterField label="Head branch">
                <Input
                  value={filters.head}
                  onChange={(e) => setField("head", e.target.value)}
                  placeholder="feature/x"
                />
              </FilterField>

              <FilterField
                label="Search"
                className="sm:col-span-2 lg:col-span-2"
                hint="Full GitHub search syntax, e.g. review:required -label:wip"
              >
                <Input
                  value={filters.search}
                  onChange={(e) => setField("search", e.target.value)}
                  placeholder="review:required in:title fix"
                />
              </FilterField>

              <FilterField label="Limit">
                <Input
                  type="number"
                  min={1}
                  max={1000}
                  value={filters.limit}
                  onChange={(e) =>
                    setField("limit", Number(e.target.value) || 30)
                  }
                />
              </FilterField>
            </div>

            <div className="mt-4 flex items-center gap-3">
              <label className="flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={filters.draftOnly}
                  onChange={(e) => setField("draftOnly", e.target.checked)}
                  className="size-4 rounded border-input"
                />
                Drafts only
              </label>
              <div className="ml-auto flex gap-2">
                <Button
                  type="button"
                  variant="ghost"
                  onClick={resetFilters}
                >
                  <X />
                  Reset
                </Button>
                <Button type="button" onClick={commit}>
                  Apply filters
                </Button>
              </div>
            </div>
          </div>
        )}
      </div>

      {isLoading && searchRepo && (
        <div className="flex flex-col gap-2">
          {Array.from({ length: 8 }).map((_, i) => (
            <Skeleton key={i} className="h-12 w-full" />
          ))}
        </div>
      )}

      {error && (
        <div className="rounded-md border border-destructive p-4 text-destructive">
          {error instanceof Error ? error.message : "Failed to fetch PRs"}
        </div>
      )}

      {ciMutation.isError && (
        <div className="rounded-md border border-destructive p-4 text-destructive">
          {ciMutation.error instanceof Error
            ? ciMutation.error.message
            : "Failed to update CI label"}
        </div>
      )}

      {prs && (
        <div className="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-16">#</TableHead>
                <TableHead>Title</TableHead>
                <TableHead className="w-32">Author</TableHead>
                <TableHead className="w-32">Branch</TableHead>
                <TableHead className="w-24">Status</TableHead>
                <TableHead className="w-28">Date</TableHead>
                <TableHead className="w-12 text-center">CI</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {prs.length === 0 && (
                <TableRow>
                  <TableCell
                    colSpan={7}
                    className="text-center text-muted-foreground"
                  >
                    No pull requests match these filters.
                  </TableCell>
                </TableRow>
              )}
              {prs.map((pr) => {
                const badge = stateBadge(pr);
                const hasCi = pr.labels.some(
                  (l) => l.toLowerCase() === CI_LABEL.toLowerCase(),
                );
                const isCiPending =
                  ciMutation.isPending && ciMutation.variables === pr.number;
                return (
                  <TableRow key={pr.number}>
                    <TableCell className="font-mono text-muted-foreground">
                      {pr.number}
                    </TableCell>
                    <TableCell>
                      <a
                        href={pr.url}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="font-medium hover:underline"
                      >
                        {pr.title}
                      </a>
                    </TableCell>
                    <TableCell className="text-muted-foreground">
                      {pr.author}
                    </TableCell>
                    <TableCell>
                      <code className="rounded bg-muted px-1.5 py-0.5 text-xs">
                        {pr.headRefName}
                      </code>
                    </TableCell>
                    <TableCell>
                      <Badge variant={badge.variant}>{badge.label}</Badge>
                    </TableCell>
                    <TableCell className="text-muted-foreground">
                      {formatDate(pr.createdAt)}
                    </TableCell>
                    <TableCell className="text-center">
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        onClick={() => ciMutation.mutate(pr.number)}
                        disabled={isCiPending}
                        aria-pressed={hasCi}
                        title={
                          hasCi
                            ? "Re-add CI label (removes then adds it again)"
                            : "Add CI label"
                        }
                        className={
                          hasCi
                            ? "text-blue-500"
                            : "opacity-40 hover:opacity-100"
                        }
                      >
                        {isCiPending ? (
                          <Loader2 className="animate-spin" />
                        ) : (
                          <FlaskConical />
                        )}
                      </Button>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </div>
      )}

      {prs && (
        <p className="text-sm text-muted-foreground">
          {prs.length} pull request{prs.length !== 1 ? "s" : ""}
          {activeCount > 0 ? " (filtered)" : ""}
        </p>
      )}
    </div>
  );
}

function FilterField({
  label,
  hint,
  className,
  children,
}: {
  label: string;
  hint?: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <div className={`flex flex-col gap-1.5 ${className ?? ""}`}>
      <label className="text-xs font-medium text-muted-foreground">
        {label}
      </label>
      {children}
      {hint && <p className="text-xs text-muted-foreground">{hint}</p>}
    </div>
  );
}

export const pullRequestsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/pull-requests",
  component: PullRequestsPage,
});
