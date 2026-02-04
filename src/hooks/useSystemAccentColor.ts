import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/**
 * Convert hex color to RGB components
 */
function hexToRgb(hex: string): { r: number; g: number; b: number } | null {
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
  return result
    ? {
        r: parseInt(result[1], 16),
        g: parseInt(result[2], 16),
        b: parseInt(result[3], 16),
      }
    : null;
}

/**
 * Apply accent color CSS variables to the document
 */
function applyAccentColor(hexColor: string) {
  const rgb = hexToRgb(hexColor);
  if (!rgb) return;

  const { r, g, b } = rgb;
  const root = document.documentElement;

  // Main accent color
  root.style.setProperty("--pp-accent", `rgb(${r}, ${g}, ${b})`);
  
  // Border variant (45% opacity)
  root.style.setProperty("--pp-accent-border", `rgba(${r}, ${g}, ${b}, 0.45)`);
  
  // Soft background variant (14% opacity)
  root.style.setProperty("--pp-accent-soft", `rgba(${r}, ${g}, ${b}, 0.14)`);
  
  // Glow variant for midnight theme (25% opacity)
  root.style.setProperty("--pp-accent-glow", `rgba(${r}, ${g}, ${b}, 0.25)`);

  console.log("[powerpaste] applied accent color:", hexColor);
}

/**
 * Hook to sync system accent color to CSS variables.
 * Fetches initial color on mount and listens for live updates.
 */
export function useSystemAccentColor() {
  useEffect(() => {
    if (typeof window === "undefined" || !(window as unknown as { __TAURI__?: unknown }).__TAURI__) {
      return;
    }

    // Fetch initial accent color
    invoke<string>("get_system_accent_color")
      .then((color) => {
        applyAccentColor(color);
      })
      .catch((err) => {
        console.error("[powerpaste] failed to get system accent color:", err);
      });

    // Listen for live accent color changes
    const unlisten = listen<string>("system-accent-changed", (event) => {
      applyAccentColor(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);
}
