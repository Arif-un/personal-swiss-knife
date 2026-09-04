import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Rocket, RotateCcw } from "lucide-react";
import { Button } from "#components/ui/button.tsx";
import { Popover, PopoverContent, PopoverTrigger } from "#components/ui/popover.tsx";
import { Tooltip, TooltipContent, TooltipTrigger } from "#components/ui/tooltip.tsx";
import { wpDeployApi, wpDeployKeys } from "#components/submodules/deployApi.ts";
import { sshApi, sshKeys } from "#components/ssh/api.ts";

/**
 * Per-repo deploy control. Loads the repo's deployable products; repos with no
 * mapping show a "configure in Settings" hint. One product deploys directly,
 * several open a list.
 * Build assets toggle applies to envira/soliloquy/cdn (ignored elsewhere).
 */
export function DeployButton({
  repo,
  enviraDev,
  configured,
  busy,
  onDeploy,
  onRollback,
  onOpenSettings,
}: {
  repo: string;
  enviraDev: string;
  configured: boolean;
  busy: boolean;
  onDeploy: (slug: string, build: boolean) => void;
  onRollback: (slug: string) => void;
  onOpenSettings: () => void;
}) {
  const [open, setOpen] = useState(false);
  const [build, setBuild] = useState(false);
  // A pending deploy/rollback awaiting confirmation — deploy overwrites a live
  // site with --force, so it is gated behind an explicit second click that also
  // names the target host (below), rather than firing straight from one tap.
  const [pending, setPending] = useState<{ kind: "deploy" | "rollback"; slug: string } | null>(
    null,
  );

  const {
    data: products,
    isError: productsError,
    error: productsErr,
  } = useQuery({
    queryKey: wpDeployKeys.products(enviraDev, repo),
    queryFn: () => wpDeployApi.products(enviraDev, repo),
    enabled: enviraDev.trim().length > 0,
    staleTime: Infinity,
  });
  const { data: config } = useQuery({
    queryKey: wpDeployKeys.config(),
    queryFn: wpDeployApi.configGet,
  });
  const { data: hosts } = useQuery({
    queryKey: sshKeys.hosts(),
    queryFn: sshApi.hostsList,
  });
  const targetHost = hosts?.find((h) => h.id === config?.targetHostId);
  const hostLabel = targetHost ? targetHost.alias || targetHost.hostname : "the target host";

  // Whether this product needs a separate asset build (nextgen + theme build
  // during the zip step). Decided by the backend so the vocabulary lives in one
  // place instead of a hardcoded list here that drifts from the deploy logic.
  const buildable = !!products?.[0]?.buildable;

  function confirmPending() {
    if (!pending) return;
    const p = pending;
    setPending(null);
    setOpen(false);
    if (p.kind === "deploy") onDeploy(p.slug, buildable && build);
    else onRollback(p.slug);
  }

  return (
    <Popover
      open={open}
      onOpenChange={(v) => {
        setOpen(v);
        if (!v) setPending(null);
      }}
    >
      <Tooltip>
        <TooltipTrigger
          render={
            <PopoverTrigger
              render={
                <Button variant="ghost" size="icon-sm" disabled={busy} aria-label="Build & deploy">
                  <Rocket />
                </Button>
              }
            />
          }
        />
        <TooltipContent>Build &amp; deploy</TooltipContent>
      </Tooltip>
      <PopoverContent align="end" className="w-80 p-3">
        {!configured ? (
          <div className="flex flex-col gap-2">
            <p className="text-sm text-muted-foreground">
              Set the target host, docroot and zip base first.
            </p>
            <Button
              size="sm"
              onClick={() => {
                setOpen(false);
                onOpenSettings();
              }}
            >
              Open deploy settings
            </Button>
          </div>
        ) : productsError ? (
          <div className="flex flex-col gap-2">
            <p className="text-sm text-destructive">
              {productsErr instanceof Error ? productsErr.message : "Failed to load products."}
            </p>
            <Button
              size="sm"
              onClick={() => {
                setOpen(false);
                onOpenSettings();
              }}
            >
              Open deploy settings
            </Button>
          </div>
        ) : !products ? (
          <p className="text-sm text-muted-foreground">Loading…</p>
        ) : products.length === 0 ? (
          <div className="flex flex-col gap-2">
            <p className="text-sm text-muted-foreground">
              No products mapped for <span className="font-mono">{repo}</span>. Add it to the
              product map in Settings.
            </p>
            <Button
              size="sm"
              onClick={() => {
                setOpen(false);
                onOpenSettings();
              }}
            >
              Open deploy settings
            </Button>
          </div>
        ) : pending ? (
          <div className="flex flex-col gap-3">
            <p className="text-sm font-medium">
              {pending.kind === "deploy" ? "Deploy" : "Roll back"}{" "}
              <span className="font-mono">{pending.slug}</span>?
            </p>
            <p className="text-xs text-muted-foreground">
              {pending.kind === "deploy"
                ? `Overwrites the live plugin/theme on ${hostLabel}.`
                : `Restores the last backup on ${hostLabel}.`}
            </p>
            <div className="flex gap-2">
              <Button
                size="sm"
                variant="outline"
                className="flex-1"
                onClick={() => setPending(null)}
              >
                Cancel
              </Button>
              <Button size="sm" className="flex-1" onClick={confirmPending}>
                Confirm
              </Button>
            </div>
          </div>
        ) : (
          <div className="flex flex-col gap-3">
            <p className="text-sm font-medium">Build &amp; deploy</p>
            <p className="text-xs text-muted-foreground">
              Target: <span className="font-mono">{hostLabel}</span>
            </p>

            {buildable ? (
              <label className="flex items-start gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={build}
                  onChange={(e) => setBuild(e.target.checked)}
                  className="mt-0.5"
                />
                <span>
                  Build assets first
                  <span className="block text-xs text-muted-foreground">
                    Only for JS/SCSS edits. Skip for PHP-only.
                  </span>
                </span>
              </label>
            ) : (
              <p className="text-xs text-muted-foreground">
                Assets build automatically while packaging.
              </p>
            )}

            {products.length === 1 ? (
              <div className="flex items-center gap-2">
                <Button
                  size="sm"
                  className="flex-1"
                  disabled={busy}
                  onClick={() => setPending({ kind: "deploy", slug: products[0].slug })}
                >
                  <Rocket className="size-3.5" />
                  Deploy {products[0].slug}
                </Button>
                <Button
                  size="icon-sm"
                  variant="outline"
                  disabled={busy}
                  aria-label="Rollback"
                  title="Rollback to last backup"
                  onClick={() => setPending({ kind: "rollback", slug: products[0].slug })}
                >
                  <RotateCcw />
                </Button>
              </div>
            ) : (
              <div className="max-h-72 overflow-y-auto rounded-md border">
                {products.map((p) => (
                  <div
                    key={p.slug}
                    className="flex items-center gap-2 border-b px-2 py-1.5 last:border-b-0"
                  >
                    <span className="min-w-0 flex-1 truncate font-mono text-xs" title={p.slug}>
                      {p.slug}
                    </span>
                    <Button
                      size="sm"
                      disabled={busy}
                      onClick={() => setPending({ kind: "deploy", slug: p.slug })}
                    >
                      Deploy
                    </Button>
                    <Button
                      size="icon-sm"
                      variant="outline"
                      disabled={busy}
                      aria-label="Rollback"
                      title="Rollback to last backup"
                      onClick={() => setPending({ kind: "rollback", slug: p.slug })}
                    >
                      <RotateCcw />
                    </Button>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
}
