/**
 * House icon geometry, kept out of ui/icons.tsx so that module re-exports
 * components and nothing else (fast-refresh boundary hygiene).
 */

/** House defaults: 16px, stroke 1.5 — the IDE register. */
export const ICON: { size: number; strokeWidth: number } = { size: 16, strokeWidth: 1.5 };

/** Slightly larger, for empty-state cards. */
export const ICON_LG: { size: number; strokeWidth: number } = { size: 32, strokeWidth: 1.25 };
