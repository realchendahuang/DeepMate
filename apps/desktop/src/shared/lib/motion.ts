import type { Transition, Variants } from "motion/react";

// JS-driven motion tokens for the Motion library (`motion/react`). Durations
// and easings are design tokens like the CSS ones — see
// docs/DESIGN_SYSTEM.md "Motion". Two speeds, no bounce, no decoration.
export const MOTION_DURATION = {
  /** State feedback, list reflow, page exits. Matches the CSS default. */
  fast: 0.14,
  /** Page and section entrances. */
  base: 0.18,
} as const;

export const MOTION_EASE: Record<"out" | "in", [number, number, number, number]> = {
  /** Fast start, gentle settle — entrances. */
  out: [0.16, 1, 0.3, 1],
  /** Gentle start, quick stop — exits. */
  in: [0.5, 0, 0.75, 0],
};

/** Delay between children of a staggered container. */
export const STAGGER = 0.035;

export const enterTransition: Transition = {
  duration: MOTION_DURATION.base,
  ease: MOTION_EASE.out,
};

export const exitTransition: Transition = {
  duration: MOTION_DURATION.fast,
  ease: MOTION_EASE.in,
};

// Page-level view switch. `direction` is +1 when moving forward in the nav
// order: the new page enters from the right, the old one leaves to the left.
export const pageVariants: Variants = {
  enter: (direction: number) => ({ opacity: 0, x: direction * 24 }),
  center: { opacity: 1, x: 0, transition: enterTransition },
  exit: (direction: number) => ({ opacity: 0, x: direction * -16, transition: exitTransition }),
};

// One block fading up, used inside a `staggerContainer`.
export const fadeUp: Variants = {
  hidden: { opacity: 0, y: 8 },
  show: { opacity: 1, y: 0, transition: enterTransition },
};

/** Parent that staggers its `fadeUp` children. */
export const staggerContainer: Variants = {
  hidden: {},
  show: { transition: { staggerChildren: STAGGER } },
};
