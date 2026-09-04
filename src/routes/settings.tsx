import { useEffect, useState } from "react";
import { createRoute } from "@tanstack/react-router";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { DownloadIcon, PlusIcon, Trash2Icon, UploadIcon } from "lucide-react";
import { rootRoute } from "./__root.tsx";
import { Button } from "#components/ui/button.tsx";
import { Input } from "#components/ui/input.tsx";
import { Separator } from "#components/ui/separator.tsx";
import { ErrorBox } from "#components/pull-requests/ErrorBox.tsx";
import { useBranding } from "#hooks/use-branding.tsx";
import { awsauthApi, awsauthKeys } from "#components/awsauth/api.ts";
import { devkonApi, devkonKeys } from "#components/devkon/api.ts";
import { settingsApi, settingsKeys, type RepoMapping } from "#components/settings/api.ts";

/** A titled settings block. */
function Section({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
}) {
  return (
    <section className="flex flex-col gap-3 rounded-lg border p-4">
      <div className="flex flex-col gap-0.5">
        <h2 className="text-sm font-semibold">{title}</h2>
        {description && <p className="text-xs text-muted-foreground">{description}</p>}
      </div>
      {children}
    </section>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1 text-sm">
      <span className="text-xs font-medium text-muted-foreground">{label}</span>
      {children}
    </label>
  );
}

// -------------------------------------------------------------------- Branding
function BrandingSection() {
  const { branding, setBranding } = useBranding();
  const [displayName, setDisplayName] = useState(branding.displayName);
  const [accentColor, setAccentColor] = useState(branding.accentColor);
  useEffect(() => {
    setDisplayName(branding.displayName);
    setAccentColor(branding.accentColor);
  }, [branding]);

  const save = useMutation({
    mutationFn: () =>
      setBranding({ displayName: displayName.trim(), accentColor: accentColor.trim() }),
  });

  return (
    <Section
      title="Branding"
      description="App display name and accent colour. Applied live across the app; the OS app icon and bundle id are set at build time."
    >
      <div className="grid gap-3 sm:grid-cols-2">
        <Field label="Display name">
          <Input value={displayName} onChange={(e) => setDisplayName(e.target.value)} />
        </Field>
        <Field label="Accent colour (any CSS colour)">
          <div className="flex items-center gap-2">
            <input
              type="color"
              aria-label="Accent colour picker"
              // A color input only speaks hex; text input covers oklch/hsl/etc.
              onChange={(e) => setAccentColor(e.target.value)}
              className="h-9 w-10 shrink-0 rounded-md border bg-background p-1"
            />
            <Input value={accentColor} onChange={(e) => setAccentColor(e.target.value)} />
          </div>
        </Field>
      </div>
      <div className="flex items-center gap-2">
        <span
          className="size-6 rounded-md border"
          style={{ background: accentColor }}
          aria-hidden
        />
        <Button size="sm" disabled={save.isPending} onClick={() => save.mutate()}>
          {save.isPending ? "Saving…" : "Save branding"}
        </Button>
      </div>
      {save.isError && <ErrorBox error={save.error} fallback="Failed to save branding" />}
    </Section>
  );
}

// ------------------------------------------------------------- Backup / restore
function BackupSection() {
  const qc = useQueryClient();
  const exportAll = useMutation({ mutationFn: () => settingsApi.exportAll() });
  const importAll = useMutation({
    mutationFn: () => settingsApi.importAll(),
    onSuccess: (ok) => {
      // Restored files change nearly everything; a reload is the simplest way to
      // pull the fresh state everywhere. (ok=false means the user cancelled the
      // file picker — nothing changed, so don't reload.)
      if (ok) window.location.reload();
    },
    onError: () => {
      // A partial import (e.g. some keychain writes failed) still rewrote the config
      // files on disk before erroring, so refresh queries to reflect the new state
      // while the ErrorBox keeps the partial-failure message visible.
      // ponytail: branding is useState-backed (BrandingProvider), not a react-query
      // query, so invalidateQueries won't refresh it. On a partial failure the header
      // and window title can show the OLD branding until the next launch. Accepted:
      // it self-corrects on relaunch and the trigger (a keychain write failure) is
      // rare. Add a branding refresh here if it ever needs to update in place.
      void qc.invalidateQueries();
    },
  });

  return (
    <Section
      title="Backup & restore"
      description="Export every setting plus stored SSH passphrases to a single file, or restore from one. The file holds secrets in plain text — keep it off cloud-synced folders (Dropbox, iCloud, Drive)."
    >
      <div className="flex flex-wrap items-center gap-2">
        <Button
          size="sm"
          variant="outline"
          disabled={exportAll.isPending}
          onClick={() => exportAll.mutate()}
        >
          <DownloadIcon />
          {exportAll.isPending ? "Exporting…" : "Export…"}
        </Button>
        <Button
          size="sm"
          variant="outline"
          disabled={importAll.isPending}
          onClick={() => {
            // Import overwrites every setting; a wrong file wipes hosts, deploy
            // targets and passphrases. Backend snapshots the current state first,
            // but still confirm before the destructive restore.
            if (
              window.confirm(
                "Restore settings from a backup? This overwrites all current settings and SSH passphrases.",
              )
            ) {
              importAll.mutate();
            }
          }}
        >
          <UploadIcon />
          {importAll.isPending ? "Importing…" : "Import…"}
        </Button>
        {exportAll.isSuccess && exportAll.data && (
          <span className="text-xs text-muted-foreground">Saved to {exportAll.data}</span>
        )}
      </div>
      {(exportAll.isError || importAll.isError) && (
        <ErrorBox error={exportAll.error ?? importAll.error} fallback="Backup operation failed" />
      )}
    </Section>
  );
}

// ------------------------------------------------------------------- Deploy (devkon)
function DevkonSection() {
  const qc = useQueryClient();
  const { data } = useQuery({ queryKey: devkonKeys.list(), queryFn: () => devkonApi.list() });
  const [repo, setRepo] = useState("");
  const [workflow, setWorkflow] = useState("");
  const [clusterDomain, setClusterDomain] = useState("");
  useEffect(() => {
    if (data) {
      setRepo(data.repo);
      setWorkflow(data.workflow);
      setClusterDomain(data.clusterDomain);
    }
  }, [data]);

  const save = useMutation({
    mutationFn: () => settingsApi.setDevkon(repo.trim(), workflow.trim(), clusterDomain.trim()),
    onSuccess: () => qc.invalidateQueries({ queryKey: devkonKeys.list() }),
  });

  return (
    <Section
      title="Deploy target (Deploy page)"
      description="GitHub Actions workflow that the Deploy page dispatches via the gh CLI."
    >
      <div className="grid gap-3 sm:grid-cols-2">
        <Field label="Repo (owner/repo)">
          <Input value={repo} onChange={(e) => setRepo(e.target.value)} placeholder="owner/repo" />
        </Field>
        <Field label="Workflow file">
          <Input
            value={workflow}
            onChange={(e) => setWorkflow(e.target.value)}
            placeholder="deploy.yml"
          />
        </Field>
        <Field label="Namespace URL template ({name} is substituted)">
          <Input
            value={clusterDomain}
            onChange={(e) => setClusterDomain(e.target.value)}
            placeholder="https://{name}.example.com"
          />
        </Field>
      </div>
      <div>
        <Button size="sm" disabled={save.isPending} onClick={() => save.mutate()}>
          {save.isPending ? "Saving…" : "Save deploy target"}
        </Button>
      </div>
      {save.isError && <ErrorBox error={save.error} fallback="Failed to save deploy target" />}
    </Section>
  );
}

// ----------------------------------------------------------------- AWS login
function AwsSection() {
  const qc = useQueryClient();
  const { data } = useQuery({
    queryKey: awsauthKeys.config(),
    queryFn: () => awsauthApi.getConfig(),
  });
  const [loginUrl, setLoginUrl] = useState("");
  const [braveProfile, setBraveProfile] = useState("");
  const [repoDir, setRepoDir] = useState("");
  useEffect(() => {
    if (data) {
      setLoginUrl(data.loginUrl);
      setBraveProfile(data.braveProfile);
      setRepoDir(data.repoDir);
    }
  }, [data]);

  const save = useMutation({
    mutationFn: () =>
      awsauthApi.setConfig({
        loginUrl: loginUrl.trim(),
        braveProfile: braveProfile.trim(),
        repoDir: repoDir.trim(),
      }),
    onSuccess: () => qc.invalidateQueries({ queryKey: awsauthKeys.config() }),
  });

  return (
    <Section
      title="AWS login (Deploy page)"
      description="SAML/SSO login opened in Brave before running the repo's awsauth script."
    >
      <div className="grid gap-3 sm:grid-cols-2">
        <Field label="SAML/SSO login URL">
          <Input
            value={loginUrl}
            onChange={(e) => setLoginUrl(e.target.value)}
            placeholder="https://accounts.google.com/o/saml2/initsso?..."
          />
        </Field>
        <Field label="Brave profile">
          <Input value={braveProfile} onChange={(e) => setBraveProfile(e.target.value)} />
        </Field>
        <Field label="Repo directory (runs tools/awsauth)">
          <Input
            value={repoDir}
            onChange={(e) => setRepoDir(e.target.value)}
            placeholder="/path/to/repo"
          />
        </Field>
      </div>
      <div>
        <Button size="sm" disabled={save.isPending} onClick={() => save.mutate()}>
          {save.isPending ? "Saving…" : "Save AWS login"}
        </Button>
      </div>
      {save.isError && <ErrorBox error={save.error} fallback="Failed to save AWS login" />}
    </Section>
  );
}

// -------------------------------------------------------------------- Cisco
function CiscoSection() {
  const qc = useQueryClient();
  const { data } = useQuery({
    queryKey: settingsKeys.cisco(),
    queryFn: () => settingsApi.getCisco(),
  });
  const [orginfo, setOrginfo] = useState("");
  const [orginfoOff, setOrginfoOff] = useState("");
  const [daemonLabel, setDaemonLabel] = useState("");
  const [daemonPlist, setDaemonPlist] = useState("");
  useEffect(() => {
    if (data) {
      setOrginfo(data.orginfo);
      setOrginfoOff(data.orginfoOff);
      setDaemonLabel(data.daemonLabel);
      setDaemonPlist(data.daemonPlist);
    }
  }, [data]);

  const save = useMutation({
    mutationFn: () =>
      settingsApi.setCisco({
        orginfo: orginfo.trim(),
        orginfoOff: orginfoOff.trim(),
        daemonLabel: daemonLabel.trim(),
        daemonPlist: daemonPlist.trim(),
      }),
    onSuccess: () => qc.invalidateQueries({ queryKey: settingsKeys.cisco() }),
  });

  return (
    <Section
      title="Cisco Umbrella (Utils page)"
      description="Defaults are the standard macOS Cisco Secure Client paths; override for a non-default install."
    >
      <div className="grid gap-3 sm:grid-cols-2">
        <Field label="OrgInfo path">
          <Input value={orginfo} onChange={(e) => setOrginfo(e.target.value)} />
        </Field>
        <Field label="OrgInfo (disabled) path">
          <Input value={orginfoOff} onChange={(e) => setOrginfoOff(e.target.value)} />
        </Field>
        <Field label="Daemon launchd label">
          <Input value={daemonLabel} onChange={(e) => setDaemonLabel(e.target.value)} />
        </Field>
        <Field label="Daemon plist path">
          <Input value={daemonPlist} onChange={(e) => setDaemonPlist(e.target.value)} />
        </Field>
      </div>
      <div>
        <Button size="sm" disabled={save.isPending} onClick={() => save.mutate()}>
          {save.isPending ? "Saving…" : "Save Cisco paths"}
        </Button>
      </div>
      {save.isError && <ErrorBox error={save.error} fallback="Failed to save Cisco paths" />}
    </Section>
  );
}

// --------------------------------------------------------------- WP products
const KINDS = ["lite", "pro", "theme"] as const;

function WpProductsSection() {
  const qc = useQueryClient();
  const { data } = useQuery({ queryKey: settingsKeys.wp(), queryFn: () => settingsApi.getWp() });
  const [themeSlug, setThemeSlug] = useState("");
  const [slugsRelPath, setSlugsRelPath] = useState("");
  // `rowKey` is a client-only stable id so React keys rows by identity, not index —
  // removing a non-last row must not shift the still-mounted inputs' state/focus.
  // Stripped before save.
  const [rows, setRows] = useState<(RepoMapping & { rowKey: string })[]>([]);
  useEffect(() => {
    if (data) {
      setThemeSlug(data.themeSlug);
      setSlugsRelPath(data.slugsRelPath);
      setRows(data.repoMap.map((r) => ({ ...r, rowKey: crypto.randomUUID() })));
    }
  }, [data]);

  const setRow = (i: number, patch: Partial<RepoMapping>) =>
    setRows((prev) => prev.map((r, idx) => (idx === i ? { ...r, ...patch } : r)));
  const addRow = () =>
    setRows((prev) => [
      ...prev,
      { repo: "", group: "", kind: "lite", rowKey: crypto.randomUUID() },
    ]);
  const removeRow = (i: number) => setRows((prev) => prev.filter((_, idx) => idx !== i));

  const save = useMutation({
    mutationFn: () =>
      settingsApi.setWpProducts(
        themeSlug.trim(),
        slugsRelPath.trim(),
        rows
          .filter((r) => r.repo.trim())
          .map((r) => ({ repo: r.repo, group: r.group, kind: r.kind })),
      ),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: settingsKeys.wp() });
      // The Submodules DeployButton resolves products server-side from this map
      // with staleTime:Infinity, so it never self-refreshes — invalidate the whole
      // wpdeploy family (config + products) or a deploy ships the stale set.
      qc.invalidateQueries({ queryKey: ["wpdeploy"] });
    },
  });

  return (
    <Section
      title="WordPress deploy products (Submodules page)"
      description="Map each submodule folder to a product group. Groups must match keys in your monorepo's product-slugs JSON."
    >
      <div className="grid gap-3 sm:grid-cols-2">
        <Field label="Theme slug (blank = no theme product)">
          <Input value={themeSlug} onChange={(e) => setThemeSlug(e.target.value)} />
        </Field>
        <Field label="product-slugs JSON path (relative to monorepo)">
          <Input
            value={slugsRelPath}
            onChange={(e) => setSlugsRelPath(e.target.value)}
            placeholder="dev/utils/src/product-slugs.json"
          />
        </Field>
      </div>

      <div className="flex flex-col gap-2">
        <span className="text-xs font-medium text-muted-foreground">Repo → product-group map</span>
        {rows.length === 0 && (
          <p className="text-xs text-muted-foreground">No mappings. Add one below.</p>
        )}
        {rows.map((row, i) => (
          <div key={row.rowKey} className="flex items-center gap-2">
            <Input
              value={row.repo}
              onChange={(e) => setRow(i, { repo: e.target.value })}
              placeholder="repo folder"
              className="h-8"
            />
            <Input
              value={row.group}
              onChange={(e) => setRow(i, { group: e.target.value })}
              placeholder="group"
              className="h-8"
            />
            <select
              value={row.kind}
              onChange={(e) => setRow(i, { kind: e.target.value })}
              className="h-8 rounded-lg border bg-background px-2 text-sm"
            >
              {KINDS.map((k) => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
            <Button
              size="icon-sm"
              variant="ghost"
              className="h-8"
              title="Remove mapping"
              onClick={() => removeRow(i)}
            >
              <Trash2Icon />
            </Button>
          </div>
        ))}
        <div>
          <Button size="sm" variant="outline" onClick={addRow}>
            <PlusIcon />
            Add mapping
          </Button>
        </div>
      </div>

      <div>
        <Button size="sm" disabled={save.isPending} onClick={() => save.mutate()}>
          {save.isPending ? "Saving…" : "Save products"}
        </Button>
      </div>
      {save.isError && <ErrorBox error={save.error} fallback="Failed to save products" />}
    </Section>
  );
}

function SettingsPage() {
  return (
    <div className="flex max-w-3xl flex-col gap-6">
      <BrandingSection />
      <BackupSection />
      <Separator />
      <DevkonSection />
      <AwsSection />
      <CiscoSection />
      <WpProductsSection />
    </div>
  );
}

export const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings",
  component: SettingsPage,
});
