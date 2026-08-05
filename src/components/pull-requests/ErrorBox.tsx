/** Standard destructive error banner used across the PR feature. */
export function ErrorBox({ error, fallback }: { error: unknown; fallback: string }) {
  return (
    <div className="rounded-md border border-destructive p-4 text-destructive">
      {error instanceof Error ? error.message : fallback}
    </div>
  );
}
