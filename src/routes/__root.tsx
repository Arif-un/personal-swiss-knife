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
      <SidebarTrigger />
    </header>
  );
}

function RootLayout() {
  return (
    <TooltipProvider>
      <SidebarProvider>
        <AppSidebar />
        <SidebarInset>
          <Header />
          <div className="flex-1 p-6">
            <Outlet />
          </div>
        </SidebarInset>
      </SidebarProvider>
    </TooltipProvider>
  );
}

export const rootRoute = createRootRoute({
  component: RootLayout,
});
