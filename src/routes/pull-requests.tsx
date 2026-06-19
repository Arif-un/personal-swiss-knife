import { useState } from "react";
import { createRoute } from "@tanstack/react-router";
import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { rootRoute } from "./__root.tsx";
import { Input } from "#components/ui/input.tsx";
import { Button } from "#components/ui/button.tsx";
import { Badge } from "#components/ui/badge.tsx";
import { Skeleton } from "#components/ui/skeleton.tsx";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "#components/ui/table.tsx";

interface PullRequest {
  number: number;
  title: string;
  author: string;
  url: string;
  createdAt: string;
  headRefName: string;
  isDraft: boolean;
}

function PullRequestsPage() {
  const [repo, setRepo] = useState("");
  const [searchRepo, setSearchRepo] = useState(repo);

  const { data: prs, isLoading, error, refetch } = useQuery<PullRequest[]>({
    queryKey: ["pull-requests", searchRepo],
    queryFn: () => invoke<PullRequest[]>("fetch_pull_requests", { repo: searchRepo }),
  });

  function handleSearch(e: React.FormEvent) {
    e.preventDefault();
    setSearchRepo(repo);
  }

  function formatDate(dateStr: string) {
    return new Date(dateStr).toLocaleDateString("en-US", {
      month: "short",
      day: "numeric",
      year: "numeric",
    });
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">Pull Requests</h1>
        <Button variant="outline" size="sm" onClick={() => refetch()}>
          Refresh
        </Button>
      </div>

      <form onSubmit={handleSearch} className="flex gap-2">
        <Input
          value={repo}
          onChange={(e) => setRepo(e.target.value)}
          placeholder="owner/repo"
          className="max-w-sm"
        />
        <Button type="submit">Fetch</Button>
      </form>

      {isLoading && (
        <div className="flex flex-col gap-2">
          {Array.from({ length: 8 }).map((_, i) => (
            <Skeleton key={i} className="h-12 w-full" />
          ))}
        </div>
      )}

      {error && (
        <div className="rounded-md border border-destructive p-4 text-destructive">
          {error instanceof Error ? error.message : "Failed to fetch PRs"}
        </div>
      )}

      {prs && (
        <div className="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-16">#</TableHead>
                <TableHead>Title</TableHead>
                <TableHead className="w-32">Author</TableHead>
                <TableHead className="w-32">Branch</TableHead>
                <TableHead className="w-24">Status</TableHead>
                <TableHead className="w-28">Date</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {prs.map((pr) => (
                <TableRow key={pr.number}>
                  <TableCell className="font-mono text-muted-foreground">
                    {pr.number}
                  </TableCell>
                  <TableCell>
                    <a
                      href={pr.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="font-medium hover:underline"
                    >
                      {pr.title}
                    </a>
                  </TableCell>
                  <TableCell className="text-muted-foreground">
                    {pr.author}
                  </TableCell>
                  <TableCell>
                    <code className="rounded bg-muted px-1.5 py-0.5 text-xs">
                      {pr.headRefName}
                    </code>
                  </TableCell>
                  <TableCell>
                    <Badge variant={pr.isDraft ? "secondary" : "default"}>
                      {pr.isDraft ? "Draft" : "Open"}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-muted-foreground">
                    {formatDate(pr.createdAt)}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}

      {prs && (
        <p className="text-sm text-muted-foreground">
          {prs.length} open pull request{prs.length !== 1 ? "s" : ""}
        </p>
      )}
    </div>
  );
}

export const pullRequestsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/pull-requests",
  component: PullRequestsPage,
});
