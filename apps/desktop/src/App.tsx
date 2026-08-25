import { useEffect } from "react";
import { useStore } from "./store";
import { AppShell } from "./components/layout/app-shell";

export default function App() {
  const refreshAll = useStore((s) => s.refreshAll);
  const loadPrefs = useStore((s) => s.loadPrefs);
  const theme = useStore((s) => s.theme);

  // Load the overview and persisted preferences (language/theme) on startup.
  useEffect(() => {
    refreshAll();
    loadPrefs();
  }, [refreshAll, loadPrefs]);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const syncTheme = () => {
      const light = theme === "light" || (theme === "system" && media.matches);
      document.documentElement.classList.toggle("dark", light);
    };

    syncTheme();
    media.addEventListener("change", syncTheme);
    return () => media.removeEventListener("change", syncTheme);
  }, [theme]);

  return <AppShell />;
}
