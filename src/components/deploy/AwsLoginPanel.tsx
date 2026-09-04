import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { FolderOpen } from "lucide-react";
import { pickDirectory } from "#lib/pick-directory.ts";
import { Button } from "#components/ui/button.tsx";
import { Input } from "#components/ui/input.tsx";
import { ErrorBox } from "#components/pull-requests/ErrorBox.tsx";
import { Tooltip, TooltipContent, TooltipTrigger } from "#components/ui/tooltip.tsx";
import { awsauthApi, awsauthKeys, type AwsAuthConfig } from "#components/awsauth/api.ts";

/** AWS SAML login helper: opens Brave to the login link (two tabs), waits for the
 * manual `credentials` download, then runs `tools/awsauth` (starting Docker if
 * needed). Profile and repo dir are persisted. */
export function AwsLoginPanel() {
  const qc = useQueryClient();
  const { data: config } = useQuery({
    queryKey: awsauthKeys.config(),
    queryFn: () => awsauthApi.getConfig(),
  });
  // Local edits so typing isn't clobbered by the query; seeded once config loads.
  const [profile, setProfile] = useState("");
  const [repoDir, setRepoDir] = useState("");
  useEffect(() => {
    if (config) {
      setProfile(config.braveProfile);
      setRepoDir(config.repoDir);
    }
  }, [config]);

  const saveConfig = useMutation({
    mutationFn: (c: AwsAuthConfig) => awsauthApi.setConfig(c),
    onSuccess: () => qc.invalidateQueries({ queryKey: awsauthKeys.config() }),
  });
  const persist = () => {
    // loginUrl is edited in Settings, not here — carry it through so a save from
    // this panel never wipes it.
    const next = {
      braveProfile: profile.trim(),
      repoDir: repoDir.trim(),
      loginUrl: config?.loginUrl ?? "",
    };
    if (config && (next.braveProfile !== config.braveProfile || next.repoDir !== config.repoDir))
      saveConfig.mutate(next);
  };

  async function browseRepoDir() {
    const dir = await pickDirectory();
    if (!dir) return;
    setRepoDir(dir);
    // Don't save until config has loaded, else loginUrl (edited in Settings) gets
    // overwritten with "" — awsauth_set_config rewrites the whole file.
    if (!config) return;
    saveConfig.mutate({
      braveProfile: profile.trim(),
      repoDir: dir,
      loginUrl: config.loginUrl,
    });
  }

  // Credentials-download wait: the countdown/cancel loop lives here (frontend) so
  // it's cancellable and shows a live timer. `finish` is the Docker + awsauth tail.
  const finish = useMutation({ mutationFn: () => awsauthApi.finish() });
  const [waiting, setWaiting] = useState(false);
  const [remaining, setRemaining] = useState(0);
  const [durationSec, setDurationSec] = useState(30); // session-only, not persisted
  const [waitError, setWaitError] = useState<string | null>(null);
  const pollRef = useRef<number | null>(null);
  // True only while a wait loop is live. cancel()/unmount flips it false so an
  // already-in-flight checkFresh() can't fire finish.mutate() after the fact.
  const activeRef = useRef(false);

  const stopPoll = () => {
    if (pollRef.current !== null) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
  };
  // Disarm the loop and clear the timer on unmount.
  useEffect(
    () => () => {
      activeRef.current = false;
      stopPoll();
    },
    [],
  );

  const cancel = () => {
    activeRef.current = false;
    stopPoll();
    setWaiting(false);
    setRemaining(0);
  };

  async function start() {
    persist();
    finish.reset();
    setWaitError(null);
    const dur = Math.min(120, Math.max(5, Math.round(durationSec) || 30));
    let baseline: number | null;
    try {
      baseline = await awsauthApi.openBrave();
    } catch (e) {
      setWaitError(String(e));
      return;
    }
    const deadline = Date.now() + dur * 1000;
    activeRef.current = true;
    setWaiting(true);
    setRemaining(dur);
    let busy = false; // guard against overlapping async polls
    pollRef.current = window.setInterval(async () => {
      if (busy) return;
      busy = true;
      try {
        setRemaining(Math.max(0, Math.ceil((deadline - Date.now()) / 1000)));
        const fresh = await awsauthApi.checkFresh(baseline);
        // Cancelled/unmounted while the IPC round-trip was in flight: drop the
        // result so a stale "fresh" can't kick off Docker + awsauth after cancel.
        if (!activeRef.current) return;
        if (fresh) {
          cancel();
          finish.mutate();
        } else if (Date.now() >= deadline) {
          cancel();
          setWaitError(`credentials file was not downloaded within ${dur}s`);
        }
      } finally {
        busy = false;
      }
    }, 500);
  }

  return (
    <div className="flex flex-col gap-2 rounded-lg border p-3">
      <div className="flex flex-wrap items-center gap-2">
        <span className="mr-1 text-sm font-medium">AWS login</span>
        <Tooltip>
          <TooltipTrigger
            render={
              <Input
                value={profile}
                onChange={(e) => setProfile(e.target.value)}
                onBlur={persist}
                placeholder="Brave profile"
                aria-label="Brave profile"
                className="h-7 w-32"
              />
            }
          />
          <TooltipContent>Brave profile</TooltipContent>
        </Tooltip>
        <span className="flex gap-1">
          <Tooltip>
            <TooltipTrigger
              render={
                <Input
                  value={repoDir}
                  onChange={(e) => setRepoDir(e.target.value)}
                  onBlur={persist}
                  placeholder="/path/to/repo"
                  aria-label="Repo directory"
                  className="h-7 w-72"
                />
              }
            />
            <TooltipContent>Repo directory</TooltipContent>
          </Tooltip>
          <Button
            type="button"
            variant="outline"
            size="icon-sm"
            className="h-7"
            onClick={browseRepoDir}
            aria-label="Browse for directory"
            title="Browse"
          >
            <FolderOpen />
          </Button>
        </span>
        <Tooltip>
          <TooltipTrigger
            render={
              <Input
                type="number"
                min={5}
                max={120}
                value={durationSec}
                onChange={(e) => setDurationSec(Number(e.target.value))}
                disabled={waiting || finish.isPending}
                aria-label="Wait (seconds)"
                className="h-7 w-16"
              />
            }
          />
          <TooltipContent>Wait (seconds)</TooltipContent>
        </Tooltip>
        {waiting ? (
          <Button size="sm" variant="destructive" onClick={cancel}>
            Cancel ({remaining}s)
          </Button>
        ) : (
          <Button size="sm" disabled={finish.isPending} onClick={start}>
            {finish.isPending ? "Authenticating…" : "Login"}
          </Button>
        )}
      </div>

      <span className="text-xs text-muted-foreground">
        Opens the SAML link in Brave (two tabs). Download{" "}
        <code className="text-xs">credentials</code> to{" "}
        <code className="text-xs">~/Downloads/AWS</code> before the countdown, then runs{" "}
        <code className="text-xs">tools/awsauth</code>. Click again to cancel.
      </span>

      {finish.isSuccess && (
        <span className="flex items-center gap-1.5 text-sm">
          <span className="size-2 rounded-full bg-green-500" />
          Login succeeded.
        </span>
      )}
      {finish.isError && (
        <pre className="max-h-64 overflow-auto rounded-md border border-destructive bg-destructive/5 p-3 text-xs whitespace-pre-wrap text-destructive">
          {/* Tauri rejects with the Rust Err string (the combined awsauth log), not an Error. */}
          {finish.error instanceof Error ? finish.error.message : String(finish.error)}
        </pre>
      )}
      {waitError && <ErrorBox error={waitError} fallback="AWS login failed" />}
    </div>
  );
}
