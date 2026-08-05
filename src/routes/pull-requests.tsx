import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { createRoute } from "@tanstack/react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import {
  FlaskConical,
  Loader2,
  MessageSquare,
  RefreshCw,
  SlidersHorizontal,
  X,
} from "lucide-react";
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
import { prViewsApi } from "#components/pull-requests/api.ts";
import { ViewsMenu } from "#components/pull-requests/ViewsMenu.tsx";
import {
  emptyFilters,
  type Filters,
  type PrView,
  type PrViewsStore,
} from "#components/pull-requests/types.ts";

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

  // How many times the CI label was added to each PR (keyed by PR number).
  const prNumbers = prs?.map((p) => p.number) ?? [];
  const ciCountsQuery = useQuery<Record<string, number>>({
    queryKey: ["ci-label-counts", searchRepo, prNumbers],
    queryFn: () =>
      invoke<Record<string, number>>("fetch_ci_label_counts", {
        repo: searchRepo,
        numbers: prNumbers,
      }),
    enabled: searchRepo.trim().length > 0 && prNumbers.length > 0,
  });
  const ciCounts = ciCountsQuery.data ?? {};

  // Unresolved review threads (conversations) per PR, keyed by PR number.
  const unresolvedQuery = useQuery<Record<string, number>>({
    queryKey: ["unresolved-comment-counts", searchRepo, prNumbers],
    queryFn: () =>
      invoke<Record<string, number>>("fetch_unresolved_comment_counts", {
        repo: searchRepo,
        numbers: prNumbers,
      }),
    enabled: searchRepo.trim().length > 0 && prNumbers.length > 0,
  });
  const unresolvedCounts = unresolvedQuery.data ?? {};

  // Adds the CI label; if already present it is removed then re-added to force
  // a fresh label event. Backend returns the PR's labels after the operation.
  const ciMutation = useMutation({
    mutationFn: (number: number) =>
      invoke<string[]>("readd_ci_label", { repo: searchRepo, number }),
    onSuccess: (labels, number) => {
      queryClient.setQueryData<PullRequest[]>(prsQueryKey, (prev) =>
        prev?.map((pr) => (pr.number === number ? { ...pr, labels } : pr)),
      );
      // Each re-add produces one more "labeled" event; refresh the counts.
      queryClient.invalidateQueries({
        queryKey: ["ci-label-counts", searchRepo],
      });
    },
  });

  // ---- Saved views ----
  const viewsQuery = useQuery<PrViewsStore>({
    queryKey: ["pr-views"],
    queryFn: () => prViewsApi.list(),
  });
  const views = viewsQuery.data?.views ?? [];
  const activeViewId = viewsQuery.data?.activeViewId ?? null;

  const invalidateViews = () =>
    queryClient.invalidateQueries({ queryKey: ["pr-views"] });

  const saveViewMutation = useMutation({
    mutationFn: prViewsApi.save,
    onSuccess: invalidateViews,
  });
  const deleteViewMutation = useMutation({
    mutationFn: prViewsApi.delete,
    onSuccess: invalidateViews,
  });
  const setActiveMutation = useMutation({
    mutationFn: prViewsApi.setActive,
    onSuccess: invalidateViews,
  });
  const viewsBusy =
    viewsQuery.isLoading ||
    saveViewMutation.isPending ||
    deleteViewMutation.isPending ||
    setActiveMutation.isPending;

  // Populate the page from a view and fetch immediately.
  function loadView(view: PrView) {
    const f = { ...emptyFilters, ...view.filters };
    setRepo(view.repo);
    setFilters(f);
    setSearchRepo(view.repo);
    setAppliedFilters(f);
  }

  function applyView(view: PrView) {
    loadView(view);
    if (view.id !== activeViewId) setActiveMutation.mutate(view.id);
  }

  function saveCurrentAsView(name: string) {
    saveViewMutation.mutate({ name, repo: searchRepo, filters: appliedFilters });
  }

  function updateViewToCurrent(view: PrView) {
    saveViewMutation.mutate({
      id: view.id,
      name: view.name,
      repo: searchRepo,
      filters: appliedFilters,
    });
  }

  function renameView(view: PrView, name: string) {
    saveViewMutation.mutate({ ...view, name });
  }

  function deleteView(view: PrView) {
    deleteViewMutation.mutate(view.id);
  }

  // On first load, restore the last-active view and fetch it.
  const didInitView = useRef(false);
  useEffect(() => {
    if (didInitView.current || !viewsQuery.data) return;
    didInitView.current = true;
    const active = viewsQuery.data.views.find(
      (v) => v.id === viewsQuery.data!.activeViewId,
    );
    if (!active) return;
    const f = { ...emptyFilters, ...active.filters };
    setRepo(active.repo);
    setFilters(f);
    setSearchRepo(active.repo);
    setAppliedFilters(f);
  }, [viewsQuery.data]);

  // The header actions slot lives in the global header (see __root.tsx); we
  // portal the views menu into it so it only mounts on this route.
  const [headerSlot, setHeaderSlot] = useState<HTMLElement | null>(null);
  useEffect(() => {
    setHeaderSlot(document.getElementById("header-actions"));
  }, []);

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
      {headerSlot &&
        createPortal(
          <>
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={() => refetch()}
              disabled={!searchRepo.trim() || isFetching}
              aria-label="Refresh"
              title="Refresh"
            >
              <RefreshCw className={isFetching ? "animate-spin" : undefined} />
            </Button>
            <ViewsMenu
              views={views}
              activeViewId={activeViewId}
              canSaveCurrent={searchRepo.trim().length > 0}
              busy={viewsBusy}
              onApply={applyView}
              onSaveCurrent={saveCurrentAsView}
              onUpdate={updateViewToCurrent}
              onRename={renameView}
              onDelete={deleteView}
            />
          </>,
          headerSlot,
        )}

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
          <Table className="text-[10px]">
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
                const ciCount = ciCounts[String(pr.number)] ?? 0;
                const unresolvedCount =
                  unresolvedCounts[String(pr.number)] ?? 0;
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
                      <div className="relative inline-flex">
                        <Button
                          variant="ghost"
                          size="icon-sm"
                          onClick={() => ciMutation.mutate(pr.number)}
                          disabled={isCiPending}
                          aria-pressed={hasCi}
                          title={
                            hasCi
                              ? `Re-add CI label (added ${ciCount}× so far)`
                              : `Add CI label${ciCount ? ` (added ${ciCount}× so far)` : ""}`
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
