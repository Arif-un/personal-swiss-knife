import { Outlet, createRootRoute } from "@tanstack/react-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
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

function Header() {
  const { state } = useSidebar();

  return (
    <header
      onMouseDown={(e) => {
        if ((e.target as HTMLElement).closest("button, a, input")) return;
        getCurrentWindow().startDragging();
      }}
      className={cn(
        "flex h-12 shrink-0 items-center gap-2 border-b px-4 transition-[padding] duration-200",
        state === "collapsed" && "pl-20",
      )}
    >
      <SidebarTrigger className="size-[18px] min-w-[18px] rounded-full p-0 [&_svg]:size-3" />
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
