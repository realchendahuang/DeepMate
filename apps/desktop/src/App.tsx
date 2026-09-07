import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { MotionConfig } from "motion/react";
import { useRuntimeStore } from "./app/store/runtime";
import { useScenarioStore } from "./app/store/scenarios";
import { usePreferencesStore } from "./app/store/preferences";
import { AppShell } from "./app/shell/app-shell";
import { Toaster } from "./shared/ui/sonner";

export default function App() {
  const { i18n } = useTranslation();
  const theme = usePreferencesStore((s) => s.theme);

  // Load the engine overview, the scenario inventory and the persisted
  // preferences on startup, then check for a new release when automatic
  // update checks are enabled. The scenario list feeds the rail and sidebar.
  useEffect(() => {
    void useRuntimeStore.getState().refreshOverview();
    void useScenarioStore.getState().loadProfiles();
    void useRuntimeStore.getState().loadInstances();
    void usePreferencesStore.getState().loadPrefs().then(() => {
      if (usePreferencesStore.getState().checkUpdates) {
        void usePreferencesStore.getState().checkUpdate();
      }
    });
  }, []);

  // Keep the runtime status fresh: refresh when the window regains focus and
  // poll while the app is visible. The polling interval is skipped when the
  // document is hidden (backgrounded or minimized). Diagnostics deliberately
  // run on the diagnostics page only, not on every poll.
  useEffect(() => {
    const refresh = () => {
      if (document.visibilityState === "visible") {
        void useRuntimeStore.getState().refreshOverview();
        void useRuntimeStore.getState().loadInstances();
      }
    };
    window.addEventListener("focus", refresh);
    const timer = window.setInterval(refresh, 10_000);
    return () => {
      window.removeEventListener("focus", refresh);
      window.clearInterval(timer);
    };
  }, []);

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
