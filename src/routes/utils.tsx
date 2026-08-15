import { createRoute } from "@tanstack/react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { rootRoute } from "./__root.tsx";
import { Button } from "#components/ui/button.tsx";
import { cn } from "#lib/utils.ts";
import { ErrorBox } from "#components/pull-requests/ErrorBox.tsx";
import { utilsApi, utilsKeys, type CiscoStatus } from "#components/utils/api.ts";

// The daemon takes a second or two to spawn the agent after a toggle; keep the
// status fresh so the restart settling (or an external change) shows up on its own.
const REFETCH_MS = 4_000;

function ciscoStatusLabel(s: CiscoStatus | undefined): { text: string; dot: string } {
  if (!s) return { text: "Checking…", dot: "bg-muted-foreground" };
  if (!s.installed) return { text: "Not installed", dot: "bg-muted-foreground" };
  // Profile presence is the real on/off signal: vpnagentd respawns acumbrellaagent
  // regardless of the profile, so `running` alone would show green after a disable.
  if (!s.profilePresent) return { text: "Disabled", dot: "bg-red-500" };
  if (s.running) return { text: "Enabled · running", dot: "bg-green-500" };
  return { text: "Enabled · starting…", dot: "bg-amber-500" };
}

function UtilsPage() {
  const qc = useQueryClient();

  const { data: status } = useQuery<CiscoStatus>({
    queryKey: utilsKeys.ciscoStatus(),
    queryFn: () => utilsApi.ciscoStatus(),
    refetchInterval: REFETCH_MS,
  });

  const toggle = useMutation({
    mutationFn: (enabled: boolean) => utilsApi.ciscoSetEnabled(enabled),
    onSuccess: (fresh) => {
      // `fresh` is read right after the restart, before vpnagentd has respawned
      // the agent, so `running` may still be false. Seed it, then re-poll a couple
      // times to catch the agent coming up.
      qc.setQueryData(utilsKeys.ciscoStatus(), fresh);
      for (const ms of [1500, 3500]) {
        window.setTimeout(() => qc.invalidateQueries({ queryKey: utilsKeys.ciscoStatus() }), ms);
      }
    },
  });

  const installed = status?.installed ?? false;
  const enabled = status?.profilePresent ?? false;
  const s = ciscoStatusLabel(status);

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col gap-4 rounded-lg border p-4">
        <div className="flex items-center justify-between gap-4">
          <div className="flex flex-col gap-1">
            <span className="font-medium">Cisco Umbrella</span>
            <span className="text-sm text-muted-foreground">
              Toggle the Umbrella DNS agent (acumbrellaagent). The VPN stays connected. Requires
              your admin password.
            </span>
          </div>

          <div className="flex shrink-0 items-center gap-3">
            <span className="flex items-center gap-1.5 text-sm text-muted-foreground">
              <span className={cn("size-2 rounded-full", s.dot)} />
              {s.text}
            </span>
            <Button
              variant={enabled ? "destructive" : "default"}
              size="sm"
              disabled={!installed || toggle.isPending}
              onClick={() => toggle.mutate(!enabled)}
            >
              {toggle.isPending ? "Applying…" : enabled ? "Disable" : "Enable"}
            </Button>
          </div>
        </div>

        {toggle.isError && (
          <ErrorBox error={toggle.error} fallback="Failed to change Cisco Umbrella state" />
        )}
      </div>
    </div>
  );
}

export const utilsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/utils",
  component: UtilsPage,
});
