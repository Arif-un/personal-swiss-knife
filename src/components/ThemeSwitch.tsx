import { SunIcon, MoonIcon, MonitorIcon } from "lucide-react";
import { Button } from "#components/ui/button.tsx";
import { useTheme } from "#hooks/use-theme.tsx";

const cycle = ["light", "dark", "system"] as const;

export function ThemeSwitch() {
  const { theme, setTheme } = useTheme();
  const next = cycle[(cycle.indexOf(theme) + 1) % cycle.length];
  const Icon = theme === "light" ? SunIcon : theme === "dark" ? MoonIcon : MonitorIcon;

  return (
    <Button
      type="button"
      variant="ghost"
      size="icon-sm"
      onClick={() => setTheme(next)}
      title={`Theme: ${theme}`}
      className="text-sidebar-foreground/70 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
    >
      <Icon className="size-4" />
    </Button>
  );
}
