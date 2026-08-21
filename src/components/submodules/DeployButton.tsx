import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Rocket, RotateCcw } from "lucide-react";
import { Button } from "#components/ui/button.tsx";
import { Popover, PopoverContent, PopoverTrigger } from "#components/ui/popover.tsx";
import { Tooltip, TooltipContent, TooltipTrigger } from "#components/ui/tooltip.tsx";
import { wpDeployApi, wpDeployKeys } from "#components/submodules/deployApi.ts";

/**
 * Per-repo deploy control. Loads the repo's deployable products; repos with no
 * products render nothing. One product deploys directly, several open a list.
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

  const { data: products } = useQuery({
    queryKey: wpDeployKeys.products(enviraDev, repo),
    queryFn: () => wpDeployApi.products(enviraDev, repo),
    enabled: enviraDev.trim().length > 0,
    staleTime: Infinity,
  });

  // Repo ships nothing deployable → no icon.
  if (products && products.length === 0) return null;

  // Only envira/soliloquy/cdn have a separate asset build; nextgen + theme
  // build during the zip step, so the toggle is irrelevant there.
  const buildable = !!products && ["envira", "soliloquy", "cdn"].includes(products[0]?.group);

  function deploy(slug: string) {
    setOpen(false);
    onDeploy(slug, buildable && build);
  }
  function rollback(slug: string) {
    setOpen(false);
    onRollback(slug);
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
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
        ) : !products ? (
          <p className="text-sm text-muted-foreground">Loading…</p>
        ) : (
          <div className="flex flex-col gap-3">
            <p className="text-sm font-medium">Build &amp; deploy</p>

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
                <Button size="sm" className="flex-1" disabled={busy} onClick={() => deploy(products[0].slug)}>
                  <Rocket className="size-3.5" />
                  Deploy {products[0].slug}
                </Button>
                <Button
                  size="icon-sm"
                  variant="outline"
                  disabled={busy}
                  aria-label="Rollback"
                  title="Rollback to last backup"
                  onClick={() => rollback(products[0].slug)}
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
                    <Button size="sm" disabled={busy} onClick={() => deploy(p.slug)}>
                      Deploy
                    </Button>
                    <Button
                      size="icon-sm"
                      variant="outline"
                      disabled={busy}
                      aria-label="Rollback"
                      title="Rollback to last backup"
                      onClick={() => rollback(p.slug)}
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
