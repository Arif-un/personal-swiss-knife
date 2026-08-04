import {
  Outlet,
  createRootRoute,
  useRouterState,
} from "@tanstack/react-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ChevronRightIcon } from "lucide-react";
import { AppSidebar, navItems } from "#components/AppSidebar.tsx";
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
  useSidebar,
} from "#components/ui/sidebar.tsx";
import { TooltipProvider } from "#components/ui/tooltip.tsx";
import { ThemeProvider } from "#hooks/use-theme.tsx";
import { cn } from "#lib/utils.ts";

function Header() {
  const { state } = useSidebar();
  const pathname = useRouterState({
    select: (s) => s.location.pathname,
  });
  const title = navItems.find((item) => item.path === pathname)?.title;

  return (
    <header
      onMouseDown={(e) => {
        if ((e.target as HTMLElement).closest("button, a, input")) return;
        getCurrentWindow().startDragging();
      }}
      className={cn(
        "flex h-12 shrink-0 items-center gap-2 border-b px-4 transition-[padding] duration-200",
        state === "collapsed" && "pl-21",
      )}
    >
      {state === "collapsed" && (
        <SidebarTrigger className="size-[18px] min-w-[18px] rounded-full p-0 [&_svg]:size-3.5 -mt-0.5 cursor-pointer" />
      )}
      <span className="flex items-center gap-1 text-lg font-bold">
        Swiss Knife
        {title && title !== "Home" && (
          <span className="flex items-center gap-1 font-normal text-muted-foreground">
            <ChevronRightIcon className="size-4" />
            {title}
          </span>
        )}
      </span>
    </header>
  );
}

function RootLayout() {
  return (
    <ThemeProvider>
      <TooltipProvider>
        <SidebarProvider>
          <AppSidebar />
          <SidebarInset className="min-w-0 overflow-x-hidden bg-background">
            <Header />
            <div className="min-w-0 flex-1 px-4 py-6">
              <Outlet />
            </div>
          </SidebarInset>
        </SidebarProvider>
      </TooltipProvider>
    </ThemeProvider>
  );
}

export const rootRoute = createRootRoute({
  component: RootLayout,
});
