import { X } from "lucide-react";
import { Button } from "#components/ui/button.tsx";
import { Input } from "#components/ui/input.tsx";
import { DEFAULT_LIMIT, type Filters } from "./types.ts";

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
      <label className="text-xs font-medium text-muted-foreground">{label}</label>
      {children}
      {hint && <p className="text-xs text-muted-foreground">{hint}</p>}
    </div>
  );
}

// The plain text-input filters, in display order. State/Search/Limit render
// specially below, so they are not in this list.
const TEXT_FIELDS: {
  key: keyof Filters;
  label: string;
  placeholder: string;
}[] = [
  { key: "author", label: "Author", placeholder: "@me or username" },
  { key: "assignee", label: "Assignee", placeholder: "@me or username" },
  { key: "labels", label: "Labels", placeholder: "bug, enhancement" },
  { key: "base", label: "Base branch", placeholder: "main" },
  { key: "head", label: "Head branch", placeholder: "feature/x" },
];

interface PrFiltersPanelProps {
  filters: Filters;
  onField: <K extends keyof Filters>(key: K, value: Filters[K]) => void;
  onReset: () => void;
  onApply: () => void;
}

export function PrFiltersPanel({
  filters,
  onField,
  onReset,
  onApply,
}: PrFiltersPanelProps) {
  return (
    <div className="rounded-md border p-4">
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <FilterField label="State">
          <select
            value={filters.state}
            onChange={(e) => onField("state", e.target.value as Filters["state"])}
            className="h-7 w-full rounded-md border border-input bg-transparent px-2 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 dark:bg-input/30"
          >
            <option value="open">Open</option>
            <option value="closed">Closed</option>
            <option value="merged">Merged</option>
            <option value="all">All</option>
          </select>
        </FilterField>

        {TEXT_FIELDS.map((field) => (
          <FilterField key={field.key} label={field.label}>
            <Input
              value={filters[field.key] as string}
              onChange={(e) => onField(field.key, e.target.value as Filters[typeof field.key])}
              placeholder={field.placeholder}
            />
          </FilterField>
        ))}

        <FilterField
          label="Search"
          className="sm:col-span-2 lg:col-span-2"
          hint="Full GitHub search syntax, e.g. review:required -label:wip"
        >
          <Input
            value={filters.search}
            onChange={(e) => onField("search", e.target.value)}
            placeholder="review:required in:title fix"
          />
        </FilterField>

        <FilterField label="Limit">
          <Input
            type="number"
            min={1}
            max={1000}
            value={filters.limit}
            onChange={(e) => onField("limit", Number(e.target.value) || DEFAULT_LIMIT)}
          />
        </FilterField>
      </div>

      <div className="mt-4 flex items-center gap-3">
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={filters.draftOnly}
            onChange={(e) => onField("draftOnly", e.target.checked)}
            className="size-4 rounded border-input"
          />
          Drafts only
        </label>
        <div className="ml-auto flex gap-2">
          <Button type="button" variant="ghost" onClick={onReset}>
            <X />
            Reset
          </Button>
          <Button type="button" onClick={onApply}>
            Apply filters
          </Button>
        </div>
      </div>
    </div>
  );
}
