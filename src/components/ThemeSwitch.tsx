import { SunIcon, MoonIcon, MonitorIcon } from "lucide-react"
import { useTheme } from "#hooks/use-theme.tsx"

const cycle = ["light", "dark", "system"] as const

export function ThemeSwitch() {
  const { theme, setTheme } = useTheme()
  const next = cycle[(cycle.indexOf(theme) + 1) % cycle.length]
  const Icon = theme === "light" ? SunIcon : theme === "dark" ? MoonIcon : MonitorIcon

  return (
    <button
      type="button"
      onClick={() => setTheme(next)}
      className="flex size-7 items-center justify-center rounded-md text-sidebar-foreground/70 transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
      title={`Theme: ${theme}`}
    >
      <Icon className="size-4" />
    </button>
  )
}
