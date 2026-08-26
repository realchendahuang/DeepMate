import type { Config } from "tailwindcss";
import animate from "tailwindcss-animate";

// DeepMate design tokens, ported from the Slint theme (ui/theme/Colors.slint).
// Dark is the default; the `.light` class flips to light, driven by the theme
// preference (system / light / dark).
export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // Surfaces, from the window up to elevated controls.
        bg: "hsl(var(--bg))",
        sidebar: "hsl(var(--sidebar))",
        panel: "hsl(var(--panel))",
        "panel-2": "hsl(var(--panel-2))",
        inset: "hsl(var(--inset))",
        hover: "hsl(var(--hover))",
        raised: "hsl(var(--raised))",
        // Hairlines and control outlines.
        border: "hsl(var(--border))",
        "border-strong": "hsl(var(--border-strong))",
        // Text hierarchy.
        text: "hsl(var(--text))",
        "text-dim": "hsl(var(--text-dim))",
        "text-faint": "hsl(var(--text-faint))",
        // Brand accent.
        accent: "hsl(var(--accent))",
        "accent-soft": "hsl(var(--accent-soft))",
        "on-accent": "hsl(var(--on-accent))",
        // Semantic tones.
        pass: "hsl(var(--pass))",
        warn: "hsl(var(--warn))",
        fail: "hsl(var(--fail))",
        skip: "hsl(var(--skip))",
        neutral: "hsl(var(--neutral))",
      },
      width: {
        sidebar: "200px",
      },
      borderRadius: {
        lg: "12px",
        md: "8px",
        sm: "6px",
      },
      fontSize: {
        display: ["22px", { lineHeight: "1.2", fontWeight: "700" }],
        title: ["17px", { lineHeight: "1.3", fontWeight: "700" }],
        heading: ["14px", { lineHeight: "1.4", fontWeight: "600" }],
        body: ["13px", { lineHeight: "1.5" }],
        small: ["12px", { lineHeight: "1.5" }],
        caption: ["11px", { lineHeight: "1.4", fontWeight: "600" }],
        brand: ["15px", { lineHeight: "1.2", fontWeight: "700" }],
      },
      boxShadow: {
        card: "0 2px 12px hsl(var(--shadow))",
        "accent-glow": "0 2px 10px hsl(var(--accent) / 0.30)",
      },
    },
  },
  plugins: [animate],
} satisfies Config;
