import { createRoute, Link } from "@tanstack/react-router";
import { rootRoute } from "./__root.tsx";
import { navItems } from "#lib/nav.ts";

function HomePage() {
  const tools = navItems.filter((item) => item.description);

  return (
    <div className="flex flex-col gap-3">
      <h1 className="text-xl font-bold">Welcome to Swiss Knife</h1>
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-3">
        {tools.map((tool) => (
          <Link
            key={tool.path}
            to={tool.path}
            className="flex items-start gap-3 rounded-lg border p-3 transition-colors hover:bg-accent"
          >
            <tool.icon className="mt-0.5 size-5 shrink-0 text-muted-foreground" />
            <div className="flex flex-col gap-0.5">
              <span className="font-medium leading-tight">{tool.title}</span>
              <span className="text-sm text-muted-foreground leading-snug">{tool.description}</span>
            </div>
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
