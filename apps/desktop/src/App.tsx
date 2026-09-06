import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { MotionConfig } from "motion/react";
import { useStore } from "./store";
import { AppShell } from "./components/layout/app-shell";
import { Toaster } from "./components/ui/sonner";

export default function App() {
  const { i18n } = useTranslation();
  const refreshAll = useStore((s) => s.refreshAll);
  const loadPrefs = useStore((s) => s.loadPrefs);
  const checkUpdate = useStore((s) => s.checkUpdate);
  const loadProfiles = useStore((s) => s.loadProfiles);
  const loadInstances = useStore((s) => s.loadInstances);
  const theme = useStore((s) => s.theme);

  // Load the overview, the scenario inventory and persisted preferences
  // (language/theme) on startup, then check for a new release when automatic
  // update checks are enabled. The scenario list feeds the sidebar group.
  useEffect(() => {
    refreshAll();
    loadProfiles();
    loadInstances();
    loadPrefs().then(() => {
      if (useStore.getState().checkUpdates) {
        checkUpdate();
      }
    });
  }, [refreshAll, loadProfiles, loadInstances, loadPrefs, checkUpdate]);

  // Keep the runtime status fresh: refresh when the window regains focus and
  // poll while the app is visible. The polling interval is skipped when the
  // document is hidden (backgrounded or minimized).
  useEffect(() => {
    const refresh = () => {
      if (document.visibilityState === "visible") {
        refreshAll();
        loadInstances();
      }
    };
    window.addEventListener("focus", refresh);
    const timer = window.setInterval(refresh, 10_000);
    return () => {
      window.removeEventListener("focus", refresh);
      window.clearInterval(timer);
    };
  }, [refreshAll]);

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

  // The document language follows the UI language for screen readers.
  useEffect(() => {
    document.documentElement.lang = i18n.language;
  }, [i18n.language]);

  return (
    <MotionConfig reducedMotion="user">
      <AppShell />
      <Toaster />
    </MotionConfig>
  );
}
