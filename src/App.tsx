import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider, createRouter } from "@tanstack/react-router";
import { rootRoute } from "./routes/__root.tsx";
import { deployRoute } from "./routes/deploy.tsx";
import { indexRoute } from "./routes/index.tsx";
import { memoryRoute } from "./routes/memory.tsx";
import { messengerRoute } from "./routes/messenger.tsx";
import { pullRequestsRoute } from "./routes/pull-requests.tsx";
import { settingsRoute } from "./routes/settings.tsx";
import { sshRoute } from "./routes/ssh.tsx";
import { submodulesRoute } from "./routes/submodules.tsx";
import { utilsRoute } from "./routes/utils.tsx";
import "./App.css";

const routeTree = rootRoute.addChildren([
  indexRoute,
  pullRequestsRoute,
  submodulesRoute,
  sshRoute,
  messengerRoute,
  memoryRoute,
  utilsRoute,
  deployRoute,
  settingsRoute,
]);

const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Desktop app hitting local CLIs: don't refetch on focus, keep results
      // briefly fresh, and retry once. Views that need always-fresh data (e.g.
      // PR checks) opt out explicitly.
      staleTime: 30_000,
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
}

export default App;
