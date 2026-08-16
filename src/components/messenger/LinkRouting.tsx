import { invoke } from "@tauri-apps/api/core";
import { GlobeIcon, Link2Icon, PlusIcon, RouteIcon, XIcon } from "lucide-react";
import type { ComponentType } from "react";
import { useEffect, useState } from "react";
import { Button } from "#components/ui/button.tsx";
import { SettingRow } from "#components/messenger/SettingRows.tsx";

type LinkAction = "same-window" | "child-webview" | "system-browser";
type LinkOverride = {
  meta: boolean;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  action: LinkAction;
};
type LinkRules = { facebook: LinkAction; other: LinkAction; overrides: LinkOverride[] };

const LINK_ACTIONS: { value: LinkAction; label: string }[] = [
  { value: "child-webview", label: "Child webview" },
  { value: "system-browser", label: "System browser" },
  { value: "same-window", label: "Same window" },
];

const MODS: { key: "meta" | "ctrl" | "alt" | "shift"; sym: string; label: string }[] = [
  { key: "meta", sym: "⌘", label: "Command" },
  { key: "ctrl", sym: "⌃", label: "Control" },
  { key: "alt", sym: "⌥", label: "Option" },
  { key: "shift", sym: "⇧", label: "Shift" },
];

/** Native select styled to match Input, for picking a link action. */
function ActionSelect({
  value,
  onChange,
}: {
  value: LinkAction;
  onChange: (a: LinkAction) => void;
}) {
  return (
    <select
      className="h-7 rounded-md border border-input bg-transparent px-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 dark:bg-input/30"
      value={value}
      onChange={(e) => onChange(e.target.value as LinkAction)}
    >
      {LINK_ACTIONS.map((a) => (
        <option key={a.value} value={a.value}>
          {a.label}
        </option>
      ))}
    </select>
  );
}

/** A destination default row: icon + label on the left, action select on the right. */
function DestRow({
  icon,
  label,
  hint,
  value,
  onChange,
}: {
  icon: ComponentType<{ className?: string }>;
  label: string;
  hint?: string;
  value: LinkAction;
  onChange: (a: LinkAction) => void;
}) {
  return (
    <SettingRow icon={icon} label={label} hint={hint}>
      <ActionSelect value={value} onChange={onChange} />
    </SettingRow>
  );
}

/** Editable link-routing rules: per-destination defaults plus modifier overrides
 *  that win over them. Persisted in Rust (`messenger_*_link_rules`). */
export function LinkRoutingSection() {
  const [rules, setRules] = useState<LinkRules | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<LinkRules>("messenger_get_link_rules")
      .then(setRules)
      .catch((e) => setError(String(e)));
  }, []);

  const save = (next: LinkRules) => {
    setRules(next);
    invoke("messenger_set_link_rules", { rules: next }).catch((e) => setError(String(e)));
  };

  if (!rules) return null;

  const setDest = (key: "facebook" | "other", action: LinkAction) =>
    save({ ...rules, [key]: action });
  const patchOverride = (i: number, patch: Partial<LinkOverride>) =>
    save({
      ...rules,
      overrides: rules.overrides.map((o, j) => (j === i ? { ...o, ...patch } : o)),
    });
  const removeOverride = (i: number) =>
    save({ ...rules, overrides: rules.overrides.filter((_, j) => j !== i) });
  const addOverride = () =>
    save({
      ...rules,
      overrides: [
        ...rules.overrides,
        { meta: true, ctrl: false, alt: false, shift: true, action: "same-window" },
      ],
    });

  return (
    <div className="rounded-lg border">
      <div className="flex items-center gap-2 px-3 py-2 text-sm font-medium">
        <RouteIcon className="size-4 text-muted-foreground" />
        Link routing
      </div>
      <div className="divide-y border-t">
        <DestRow
          icon={Link2Icon}
          label="Facebook links"
          hint="Only content links (posts, reels); chat navigation is left alone."
          value={rules.facebook}
          onChange={(a) => setDest("facebook", a)}
        />
        <DestRow
          icon={GlobeIcon}
          label="Other websites"
          value={rules.other}
          onChange={(a) => setDest("other", a)}
        />
      </div>
      <div className="border-t px-3 py-2">
        <div className="mb-2 flex items-center justify-between gap-2">
          <span className="text-xs font-medium text-muted-foreground">
            Modifier overrides (win over the defaults)
          </span>
          <Button size="sm" variant="outline" onClick={addOverride}>
            <PlusIcon className="size-3.5" />
            Add
          </Button>
        </div>
        <div className="flex flex-col gap-2">
          {rules.overrides.map((o, i) => (
            <div key={i} className="flex items-center gap-2">
              <div className="flex gap-1">
                {MODS.map((m) => (
                  <Button
                    key={m.key}
                    type="button"
                    variant={o[m.key] ? "default" : "outline"}
                    className="size-7 p-0 font-mono text-sm"
                    aria-label={m.label}
                    aria-pressed={o[m.key]}
                    onClick={() => patchOverride(i, { [m.key]: !o[m.key] })}
                  >
                    {m.sym}
                  </Button>
                ))}
              </div>
              <span className="text-muted-foreground">→</span>
              <ActionSelect value={o.action} onChange={(a) => patchOverride(i, { action: a })} />
              <Button
                size="icon"
                variant="ghost"
                className="ml-auto size-7"
                aria-label="Remove override"
                onClick={() => removeOverride(i)}
              >
                <XIcon className="size-3.5" />
              </Button>
            </div>
          ))}
          {rules.overrides.length === 0 && (
            <p className="text-xs text-muted-foreground">
              No overrides. Clicks follow the destination defaults.
            </p>
          )}
        </div>
      </div>
      {error && <p className="px-3 pb-2 text-sm text-destructive">{error}</p>}
    </div>
  );
}
