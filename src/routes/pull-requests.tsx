import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { createRoute } from "@tanstack/react-router";
import { RefreshCw, SlidersHorizontal } from "lucide-react";
import { rootRoute } from "./__root.tsx";
import { Input } from "#components/ui/input.tsx";
import { Button } from "#components/ui/button.tsx";
import { Badge } from "#components/ui/badge.tsx";
import { Skeleton } from "#components/ui/skeleton.tsx";
import { ViewsMenu } from "#components/pull-requests/ViewsMenu.tsx";
import { ErrorBox } from "#components/pull-requests/ErrorBox.tsx";
import { PrFiltersPanel } from "#components/pull-requests/PrFiltersPanel.tsx";
import { PrTable } from "#components/pull-requests/PrTable.tsx";
import { usePrAuxCounts, usePrViews, usePullRequests } from "#components/pull-requests/hooks.ts";
import { countActive } from "#components/pull-requests/utils.ts";
import { makeEmptyFilters, type Filters, type PrView } from "#components/pull-requests/types.ts";

function PullRequestsPage() {
  const [repo, setRepo] = useState("");
  const [filters, setFilters] = useState<Filters>(makeEmptyFilters);
  const [showFilters, setShowFilters] = useState(false);

  // PR numbers whose row is expanded to show CI checks. Multiple may be open.
  const [expanded, setExpanded] = useState<Set<number>>(() => new Set());
  const toggleExpand = useCallback((number: number) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(number)) next.delete(number);
      else next.add(number);
      return next;
    });
  }, []);

  // Committed values that actually drive the query.
  const [searchRepo, setSearchRepo] = useState("");
  const [appliedFilters, setAppliedFilters] = useState<Filters>(makeEmptyFilters);

  const {
    data: prs,
    isLoading,
    isFetching,
    error,
    refetch,
    ciMutation,
  } = usePullRequests(searchRepo, appliedFilters);

  const prNumbers = useMemo(() => prs?.map((p) => p.number) ?? [], [prs]);
  const { ciCounts, unresolvedCounts, mergeQueueStatus } = usePrAuxCounts(searchRepo, prNumbers);

  const {
    data: viewsData,
    views,
    activeViewId,
    busy: viewsBusy,
    saveMutation,
    deleteMutation,
    setActiveMutation,
  } = usePrViews();

  // Populate the page from a view and fetch immediately.
  const loadView = useCallback((view: PrView) => {
    const f = { ...makeEmptyFilters(), ...view.filters };
    setRepo(view.repo);
    setFilters(f);
    setSearchRepo(view.repo);
    setAppliedFilters(f);
  }, []);

  function applyView(view: PrView) {
    loadView(view);
    if (view.id !== activeViewId) setActiveMutation.mutate(view.id);
  }

  function saveCurrentAsView(name: string) {
    saveMutation.mutate({ name, repo: searchRepo, filters: appliedFilters });
  }

  function updateViewToCurrent(view: PrView) {
    saveMutation.mutate({
      id: view.id,
      name: view.name,
      repo: searchRepo,
      filters: appliedFilters,
    });
  }

  function renameView(view: PrView, name: string) {
    saveMutation.mutate({ ...view, name });
  }

  function deleteView(view: PrView) {
    deleteMutation.mutate(view.id);
  }

  // On first load, restore the last-active view and fetch it.
  const didInitView = useRef(false);
  useEffect(() => {
    if (didInitView.current || !viewsData) return;
    didInitView.current = true;
    const active = viewsData.views.find((v) => v.id === viewsData.activeViewId);
    if (active) loadView(active);
  }, [viewsData, loadView]);

  // The header actions slot lives in the global header (see __root.tsx); we
  // portal the views menu into it so it only mounts on this route.
  const [headerSlot, setHeaderSlot] = useState<HTMLElement | null>(null);
  useEffect(() => {
    setHeaderSlot(document.getElementById("header-actions"));
  }, []);

  const commit = useCallback(() => {
    setSearchRepo(repo);
    setAppliedFilters(filters);
  }, [repo, filters]);

  function handleSearch(e: React.FormEvent) {
    e.preventDefault();
    commit();
  }

  function resetFilters() {
    const empty = makeEmptyFilters();
    setFilters(empty);
    setAppliedFilters(empty);
    setSearchRepo(repo);
  }

  const setField = useCallback(<K extends keyof Filters>(key: K, value: Filters[K]) => {
    setFilters((prev) => ({ ...prev, [key]: value }));
  }, []);

  const onCiMutate = useCallback((number: number) => ciMutation.mutate(number), [ciMutation]);

  const activeCount = countActive(appliedFilters);
  const ciPendingNumber = ciMutation.isPending ? (ciMutation.variables ?? null) : null;

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
          <PrFiltersPanel
            filters={filters}
            onField={setField}
            onReset={resetFilters}
            onApply={commit}
          />
        )}
      </div>

      {isLoading && searchRepo && (
        <div className="flex flex-col gap-2">
          {Array.from({ length: 8 }).map((_, i) => (
            <Skeleton key={i} className="h-12 w-full" />
          ))}
        </div>
      )}

      {error && <ErrorBox error={error} fallback="Failed to fetch PRs" />}

      {ciMutation.isError && (
        <ErrorBox error={ciMutation.error} fallback="Failed to update CI label" />
      )}

      {prs && (
        <PrTable
          prs={prs}
          repo={searchRepo}
          expanded={expanded}
          ciCounts={ciCounts}
          unresolvedCounts={unresolvedCounts}
          mergeQueueStatus={mergeQueueStatus}
          ciPendingNumber={ciPendingNumber}
          onToggle={toggleExpand}
          onCiMutate={onCiMutate}
        />
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

export const pullRequestsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/pull-requests",
  component: PullRequestsPage,
});
