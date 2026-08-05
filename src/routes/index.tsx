import { createRoute, Link } from "@tanstack/react-router";
import { rootRoute } from "./__root.tsx";
import { navItems } from "#lib/nav.ts";

function HomePage() {
  const tools = navItems.filter((item) => item.description);

  return (
    <div className="flex flex-col gap-6">
      <h1 className="text-3xl font-bold">Welcome to Swiss Knife</h1>
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
        {tools.map((tool) => (
          <Link
            key={tool.path}
            to={tool.path}
            className="flex flex-col gap-2 rounded-lg border p-4 transition-colors hover:bg-accent"
          >
            <tool.icon className="size-5 text-muted-foreground" />
            <span className="font-medium">{tool.title}</span>
            <span className="text-sm text-muted-foreground">
              {tool.description}
            </span>
          </Link>
        ))}
      </div>
    </div>
  );
}

export const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: HomePage,
});
