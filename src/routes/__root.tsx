import { Outlet, createRootRoute, useRouterState } from "@tanstack/react-router";
import { ChevronRightIcon } from "lucide-react";
import { AppSidebar } from "#components/AppSidebar.tsx";
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
  useSidebar,
} from "#components/ui/sidebar.tsx";
import { TooltipProvider } from "#components/ui/tooltip.tsx";
import { ThemeProvider } from "#hooks/use-theme.tsx";
import { cn } from "#lib/utils.ts";
import { navItems } from "#lib/nav.ts";
import { useWindowDrag } from "#hooks/use-window-drag.ts";

function Header() {
  const { state } = useSidebar();
  const onDrag = useWindowDrag();
  const pathname = useRouterState({
    select: (s) => s.location.pathname,
  });
  const title = navItems.find((item) => item.path === pathname)?.title;

  return (
    <header
      onMouseDown={onDrag}
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
      {/* Portal target for page-specific header actions (e.g. PR views menu). */}
      <div id="header-actions" className="ml-auto flex items-center gap-2" />
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
