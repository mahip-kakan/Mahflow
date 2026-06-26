import type { Theme } from "@/bindings";

type ResolvedTheme = "light" | "dark" | "mah";

const DARK_QUERY = "(prefers-color-scheme: dark)";

/**
 * Resolve a stored theme preference to a concrete palette. "system" maps to
 * the current OS appearance (light/dark); everything else is already concrete.
 */
export function resolveTheme(theme: Theme): ResolvedTheme {
  if (theme === "system") {
    const prefersDark =
      typeof window !== "undefined" &&
      window.matchMedia(DARK_QUERY).matches;
    return prefersDark ? "dark" : "light";
  }
  return theme;
}

/** Stamp the resolved theme onto <html> as `data-theme`, which drives both the
 * CSS custom-property palettes and Tailwind's `dark:` variant. */
export function applyTheme(theme: Theme): void {
  if (typeof document === "undefined") return;
  document.documentElement.setAttribute("data-theme", resolveTheme(theme));
}

/**
 * Apply the theme now and, while it is "system", keep it in sync with OS
 * appearance changes. Returns a cleanup function for the effect.
 */
export function watchTheme(theme: Theme): () => void {
  applyTheme(theme);

  if (theme !== "system" || typeof window === "undefined") {
    return () => {};
  }

  const mql = window.matchMedia(DARK_QUERY);
  const handler = () => applyTheme("system");
  mql.addEventListener("change", handler);
  return () => mql.removeEventListener("change", handler);
}
