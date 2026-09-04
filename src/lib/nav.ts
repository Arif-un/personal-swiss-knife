import {
  ActivityIcon,
  GitBranchIcon,
  GitPullRequestIcon,
  HomeIcon,
  MessageCircleIcon,
  RocketIcon,
  SettingsIcon,
  TerminalIcon,
  WrenchIcon,
  type LucideIcon,
} from "lucide-react";

export interface NavItem {
  title: string;
  path: string;
  icon: LucideIcon;
  /** Shown on the home page tool grid (omit to hide from it). */
  description?: string;
}

/** Single source of truth for routes shown in the sidebar, header title, and
 *  home page. */
export const navItems: NavItem[] = [
  { title: "Home", path: "/", icon: HomeIcon },
  {
    title: "Pull Requests",
    path: "/pull-requests",
    icon: GitPullRequestIcon,
    description: "Browse, filter, and manage GitHub PRs across a repo.",
  },
  {
    title: "Submodules",
    path: "/submodules",
    icon: GitBranchIcon,
    description: "See and switch branches of a superproject and its submodules.",
  },
  {
    title: "SSH",
    path: "/ssh",
    icon: TerminalIcon,
    description: "Connect to hosts, open terminals, and forward ports.",
  },
  {
    title: "Messenger",
    path: "/messenger",
    icon: MessageCircleIcon,
    description: "Chat on Messenger in a light native window instead of a browser tab.",
  },
  {
    title: "Memory",
    path: "/memory",
    icon: ActivityIcon,
    description: "Track RAM usage of the app and its processes, snapshotted every 15 min.",
  },
  {
    title: "Utils",
    path: "/utils",
    icon: WrenchIcon,
    description: "System toggles like enabling or disabling Cisco Umbrella.",
  },
  {
    title: "Deploy",
    path: "/deploy",
    icon: RocketIcon,
    description: "Deploy and destroy dev clusters per name.",
  },
  {
    title: "Settings",
    path: "/settings",
    icon: SettingsIcon,
    description: "Branding, feature targets, and backup/restore of all settings.",
  },
];
