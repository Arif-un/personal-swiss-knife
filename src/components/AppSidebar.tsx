import { Link, useRouterState } from "@tanstack/react-router";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarTrigger,
} from "#components/ui/sidebar.tsx";
import { ThemeSwitch } from "#components/ThemeSwitch.tsx";
import { navItems } from "#lib/nav.ts";
import { cn } from "#lib/utils.ts";
import { isLinux } from "#lib/platform.ts";
import { useWindowDrag } from "#hooks/use-window-drag.ts";

export function AppSidebar() {
  const routerState = useRouterState();
  const currentPath = routerState.location.pathname;
  const onDrag = useWindowDrag();

  return (
    <Sidebar>
      <SidebarHeader
        className={cn("h-12 flex-row items-center py-0", isLinux ? "pl-2" : "pl-21")}
        onMouseDown={onDrag}
      >
        <SidebarTrigger className="size-[18px] min-w-[18px] -mt-0.5 rounded-full p-0 cursor-pointer hover:bg-sidebar-accent" />
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              {navItems.map((item) => (
                <SidebarMenuItem key={item.path}>
                  <SidebarMenuButton
                    isActive={currentPath === item.path}
                    render={<Link to={item.path} />}
                    tooltip={item.title}
                  >
                    <item.icon />
                    <span>{item.title}</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter>
        <ThemeSwitch />
      </SidebarFooter>
    </Sidebar>
  );
}
