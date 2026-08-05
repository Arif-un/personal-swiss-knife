import { Link, useRouterState } from "@tanstack/react-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { HomeIcon, GitPullRequestIcon, TerminalIcon } from "lucide-react";
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

export const navItems = [
  { title: "Home", path: "/", icon: HomeIcon },
  { title: "Pull Requests", path: "/pull-requests", icon: GitPullRequestIcon },
  { title: "SSH", path: "/ssh", icon: TerminalIcon },
];

export function AppSidebar() {
  const routerState = useRouterState();
  const currentPath = routerState.location.pathname;

  return (
    <Sidebar>
      <SidebarHeader
        className="h-12 flex-row items-center py-0 pl-21"
        onMouseDown={(e) => {
          if ((e.target as HTMLElement).closest("button, a, input")) return;
          getCurrentWindow().startDragging();
        }}
      >
        <SidebarTrigger className="size-[18px] min-w-[18px] -mt-0.5 rounded-full p-0 cursor-pointer hover:bg-slate-500/30" />
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
