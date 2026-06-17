import { Link, useRouterState } from "@tanstack/react-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { HomeIcon, GitPullRequestIcon } from "lucide-react";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "#components/ui/sidebar.tsx";

const navItems = [
  { title: "Home", path: "/", icon: HomeIcon },
  { title: "Pull Requests", path: "/pull-requests", icon: GitPullRequestIcon },
];

export function AppSidebar() {
  const routerState = useRouterState();
  const currentPath = routerState.location.pathname;

  return (
    <Sidebar>
      <SidebarHeader
        onMouseDown={(e) => {
          if ((e.target as HTMLElement).closest("button, a, input")) return;
          getCurrentWindow().startDragging();
        }}
      >
        <div className="mt-6 flex items-center gap-2 px-2 py-1">
          <span className="text-lg font-bold">Swiss Knife</span>
        </div>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>Navigation</SidebarGroupLabel>
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
    </Sidebar>
  );
}
