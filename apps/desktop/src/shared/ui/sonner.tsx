import * as React from "react";
import { Toaster as Sonner, type ToasterProps } from "sonner";
import {
  CircleCheckIcon,
  InfoIcon,
  TriangleAlertIcon,
  OctagonXIcon,
  Loader2Icon,
} from "lucide-react";

// Toast host. The theme follows the DeepMate light/dark state: the `.light`
// class on <html> flips the CSS variables, so sonner renders with the same
// tokens in both themes.
//
// Two details matter here and were both wrong before:
//   - the values must be *colors*, not raw HSL triplets. `--panel` is
//     `222 20% 10%`, which is not a valid `background` value, so the toast
//     painted with no background at all; `--color-panel` (defined in the
//     `@theme` block) is the `hsl(...)` form.
//   - `theme` must follow the app's own light/dark state, not the OS
//     setting: DeepMate switches with the `.light` class, and a user who
//     forced the app one way while their system is the other would otherwise
//     get toasts styled for the opposite theme.
const toasterTheme = (): "light" | "dark" => {
  if (typeof document === "undefined") return "dark";
  return document.documentElement.classList.contains("light") ? "light" : "dark";
};

const Toaster = ({ ...props }: ToasterProps) => {
  const [theme, setTheme] = React.useState<"light" | "dark">(toasterTheme);
  React.useEffect(() => {
    const observer = new MutationObserver(() => setTheme(toasterTheme()));
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    });
    return () => observer.disconnect();
  }, []);

  return (
    <Sonner
      className="toaster group"
      theme={theme}
      icons={{
        success: <CircleCheckIcon className="size-4" />,
        info: <InfoIcon className="size-4" />,
        warning: <TriangleAlertIcon className="size-4" />,
        error: <OctagonXIcon className="size-4" />,
        loading: <Loader2Icon className="size-4 animate-spin" />,
      }}
      style={
        {
          "--normal-bg": "var(--color-panel)",
          "--normal-text": "var(--color-text)",
          "--normal-border": "var(--color-border)",
          // Typed toasts keep their status colors, but taken from the
          // DeepMate tokens so they agree with the rest of the UI.
          "--success-bg": "var(--color-panel)",
          "--success-text": "var(--color-pass)",
          "--success-border": "var(--color-border)",
          "--error-bg": "var(--color-panel)",
          "--error-text": "var(--color-fail)",
          "--error-border": "var(--color-border)",
          "--warning-bg": "var(--color-panel)",
          "--warning-text": "var(--color-warn)",
          "--warning-border": "var(--color-border)",
          "--info-bg": "var(--color-panel)",
          "--info-text": "var(--color-text)",
          "--info-border": "var(--color-border)",
          "--border-radius": "var(--radius)",
        } as React.CSSProperties
      }
      toastOptions={{
        classNames: {
          toast: "cn-toast",
        },
      }}
      {...props}
    />
  );
};

export { Toaster };
