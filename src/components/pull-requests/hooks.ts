import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { prApi, prKeys, prViewsApi } from "./api.ts";
import type { Filters, PullRequest } from "./types.ts";

/** PRs for the committed repo + filters, plus the "re-add CI label" mutation. */
export function usePullRequests(searchRepo: string, appliedFilters: Filters) {
  const queryClient = useQueryClient();
  const queryKey = prKeys.list(searchRepo, appliedFilters);

  const query = useQuery<PullRequest[]>({
    queryKey,
    queryFn: () => prApi.list(searchRepo, appliedFilters),
    enabled: searchRepo.trim().length > 0,
  });

  const ciMutation = useMutation({
    mutationFn: (number: number) => prApi.readdCiLabel(searchRepo, number),
    onSuccess: (labels, number) => {
      queryClient.setQueryData<PullRequest[]>(queryKey, (prev) =>
        prev?.map((pr) => (pr.number === number ? { ...pr, labels } : pr)),
      );
      // Each re-add produces one more "labeled" event; refresh the counts.
      queryClient.invalidateQueries({
        queryKey: prKeys.ciCountsRoot(searchRepo),
      });
    },
  });

  return { ...query, ciMutation };
}

/** The three per-PR sibling metrics (CI count, unresolved threads, merge queue).
 *  All share the same enable-guard and call shape, so they go through one path. */
export function usePrAuxCounts(searchRepo: string, prNumbers: number[]) {
  const enabled = searchRepo.trim().length > 0 && prNumbers.length > 0;

  const ci = useQuery({
    queryKey: prKeys.ciCounts(searchRepo, prNumbers),
    queryFn: () => prApi.ciLabelCounts(searchRepo, prNumbers),
    enabled,
  });
  const unresolved = useQuery({
    queryKey: prKeys.unresolved(searchRepo, prNumbers),
    queryFn: () => prApi.unresolvedCounts(searchRepo, prNumbers),
    enabled,
  });
  const mergeQueue = useQuery({
    queryKey: prKeys.mergeQueue(searchRepo, prNumbers),
    queryFn: () => prApi.mergeQueueStatus(searchRepo, prNumbers),
    enabled,
  });

  return {
    ciCounts: ci.data ?? {},
    unresolvedCounts: unresolved.data ?? {},
    mergeQueueStatus: mergeQueue.data ?? {},
  };
}

/** Saved views list + the save/delete/set-active mutations and a busy flag. */
export function usePrViews() {
  const queryClient = useQueryClient();
  const viewsQuery = useQuery({
    queryKey: prKeys.views(),
    queryFn: () => prViewsApi.list(),
  });
  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey: prKeys.views() });

  const saveMutation = useMutation({
    mutationFn: prViewsApi.save,
    onSuccess: invalidate,
  });
  const deleteMutation = useMutation({
    mutationFn: prViewsApi.delete,
    onSuccess: invalidate,
  });
  const setActiveMutation = useMutation({
    mutationFn: prViewsApi.setActive,
    onSuccess: invalidate,
  });

  const busy =
    viewsQuery.isLoading ||
    saveMutation.isPending ||
    deleteMutation.isPending ||
    setActiveMutation.isPending;

  return {
    data: viewsQuery.data,
    views: viewsQuery.data?.views ?? [],
    activeViewId: viewsQuery.data?.activeViewId ?? null,
    busy,
    saveMutation,
    deleteMutation,
    setActiveMutation,
  };
}
