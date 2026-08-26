import { useEffect } from "react";
import { useStore } from "./store";
import { AppShell } from "./components/layout/app-shell";

export default function App() {
  const refreshAll = useStore((s) => s.refreshAll);
  const loadPrefs = useStore((s) => s.loadPrefs);
  const checkUpdate = useStore((s) => s.checkUpdate);
  const theme = useStore((s) => s.theme);

  // Load the overview and persisted preferences (language/theme) on startup,
  // then check for a new release when automatic update checks are enabled.
  useEffect(() => {
    refreshAll();
    loadPrefs().then(() => {
      if (useStore.getState().checkUpdates) {
        checkUpdate();
      }
    });
  }, [refreshAll, loadPrefs, checkUpdate]);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const syncTheme = () => {
      const light = theme === "light" || (theme === "system" && media.matches);
      document.documentElement.classList.toggle("light", light);
    };

    syncTheme();
    media.addEventListener("change", syncTheme);
    return () => media.removeEventListener("change", syncTheme);
  }, [theme]);

  return <AppShell />;
}
