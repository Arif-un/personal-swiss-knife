import { useState } from "react";
import { createRoute, Link } from "@tanstack/react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { rootRoute } from "./__root.tsx";
import { Button } from "#components/ui/button.tsx";
import { Input } from "#components/ui/input.tsx";
import { Table, TableBody, TableHead, TableHeader, TableRow } from "#components/ui/table.tsx";
import { ErrorBox } from "#components/pull-requests/ErrorBox.tsx";
import { devkonApi, devkonKeys } from "#components/devkon/api.ts";
import { AwsLoginPanel } from "#components/deploy/AwsLoginPanel.tsx";
import { BRANCH_LIST_ID, DeployRow } from "#components/deploy/DeployRow.tsx";

function DeployPage() {
  const qc = useQueryClient();
  const [newName, setNewName] = useState("");
  // Ids currently mid-dispatch. A Set (not the shared mutation's `variables`, which
  // only holds the latest arg) so dispatching a second row can't re-enable the first.
  const [busyIds, setBusyIds] = useState<Set<string>>(new Set());
  const markBusy = (id: string, busy: boolean) =>
    setBusyIds((prev) => {
      const next = new Set(prev);
      if (busy) next.add(id);
      else next.delete(id);
      return next;
    });
  const dispatch = (
    m: { mutate: (id: string, opts: { onSettled: () => void }) => void },
    id: string,
  ) => {
    markBusy(id, true);
    m.mutate(id, { onSettled: () => markBusy(id, false) });
  };

  const { data } = useQuery({
    queryKey: devkonKeys.list(),
    queryFn: () => devkonApi.list(),
  });
  const { data: branches } = useQuery({
    queryKey: devkonKeys.branches(),
    queryFn: () => devkonApi.branches(),
    staleTime: 5 * 60_000,
  });

  const entries = data?.entries ?? [];
  const clusterDomain = data?.clusterDomain ?? "";
  const configured = Boolean(data?.repo && data?.workflow);
  const invalidateList = () => qc.invalidateQueries({ queryKey: devkonKeys.list() });

  const save = useMutation({
    mutationFn: devkonApi.save,
    onSuccess: invalidateList,
  });
  const add = useMutation({
    mutationFn: (name: string) => devkonApi.save({ name }),
    onSuccess: () => {
      setNewName("");
      invalidateList();
    },
  });
  const remove = useMutation({
    mutationFn: devkonApi.remove,
    onSuccess: invalidateList,
  });
  const deploy = useMutation({
    mutationFn: devkonApi.deploy,
    onSuccess: (e) => {
      invalidateList();
      qc.invalidateQueries({ queryKey: devkonKeys.status(e.id) });
    },
  });
  const destroy = useMutation({
    mutationFn: devkonApi.destroy,
    onSuccess: (e) => {
      invalidateList();
      qc.invalidateQueries({ queryKey: devkonKeys.status(e.id) });
    },
  });

  return (
    <div className="flex flex-col gap-6">
      <p className="text-sm text-muted-foreground">
        Deploy and destroy isolated namespaces. Each name is substituted into your{" "}
        <code className="text-xs">{"{name}"}</code> URL template and dispatches the configured
        workflow. Set the repo, workflow, and URL template in{" "}
        <Link to="/settings" className="underline hover:text-foreground">
          Settings
        </Link>
        .
      </p>

      {!configured && (
        <p className="rounded-lg border border-dashed p-3 text-sm text-muted-foreground">
          No deploy target configured yet. Add the repo and workflow in{" "}
          <Link to="/settings" className="underline hover:text-foreground">
            Settings
          </Link>{" "}
          to enable deploys.
        </p>
      )}

      <AwsLoginPanel />

      <form
        className="flex items-center gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          const n = newName.trim();
          if (n) add.mutate(n);
        }}
      >
        <Input
          placeholder="New name (namespace)…"
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
          className="max-w-xs"
        />
        <Button type="submit" disabled={!newName.trim() || add.isPending}>
          Add
        </Button>
      </form>

      {(add.isError || save.isError || remove.isError) && (
        <ErrorBox
          error={add.error ?? save.error ?? remove.error}
          fallback="Failed to update the list"
        />
      )}
      {(deploy.isError || destroy.isError) && (
        <ErrorBox
          error={deploy.error ?? destroy.error}
          fallback="Failed to dispatch the workflow"
        />
      )}

      {/* Shared branch options for every row's <input list>. */}
      <datalist id={BRANCH_LIST_ID}>
        {(branches ?? []).map((b) => (
          <option key={b} value={b} />
        ))}
      </datalist>

      {entries.length === 0 ? (
        <p className="text-sm text-muted-foreground">No names yet. Add one above.</p>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Name</TableHead>
              <TableHead>Branch</TableHead>
              <TableHead>Mode</TableHead>
              <TableHead>Status</TableHead>
              <TableHead className="text-right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {entries.map((entry) => (
              <DeployRow
                key={entry.id}
                entry={entry}
                clusterDomain={clusterDomain}
                busy={busyIds.has(entry.id)}
                onSave={(e) => save.mutate(e)}
                onDeploy={() => dispatch(deploy, entry.id)}
                onDestroy={() => dispatch(destroy, entry.id)}
                onRemove={() => remove.mutate(entry.id)}
                removePending={remove.isPending}
              />
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );
}

export const deployRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/deploy",
  component: DeployPage,
});
