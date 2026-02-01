import { useEffect, useState } from "react";
import { getAppIconPath } from "../api";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useDominantColor } from "../hooks/useDominantColor";

interface AppIconProps {
  bundleId: string | undefined;
  appName: string | undefined;
  size?: number;
  className?: string;
  onColorExtracted?: (color: string | null) => void;
}

// Cache for app icon paths to avoid repeated lookups
const iconPathCache = new Map<string, string | null>();

export function AppIcon({ bundleId, appName, size = 20, className = "", onColorExtracted }: AppIconProps) {
  const [iconUrl, setIconUrl] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(false);
  
  // Extract dominant color from the icon
  const dominantColor = useDominantColor(iconUrl);

  // Notify parent when color is extracted
  useEffect(() => {
    if (onColorExtracted) {
      onColorExtracted(dominantColor);
    }
  }, [dominantColor, onColorExtracted]);

  useEffect(() => {
    if (!bundleId) {
      setIconUrl(null);
      return;
    }

    // Check cache first
    if (iconPathCache.has(bundleId)) {
      const cachedPath = iconPathCache.get(bundleId);
      if (cachedPath) {
        setIconUrl(convertFileSrc(cachedPath));
      } else {
        setError(true);
      }
      return;
    }

    let cancelled = false;
    setLoading(true);
    setError(false);

    getAppIconPath(bundleId)
      .then((path) => {
        if (cancelled) return;
        iconPathCache.set(bundleId, path);
        if (path) {
          setIconUrl(convertFileSrc(path));
        } else {
          setError(true);
        }
        setLoading(false);
      })
      .catch(() => {
        if (cancelled) return;
        iconPathCache.set(bundleId, null);
        setError(true);
        setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [bundleId]);

  // If no bundle ID or still loading, show nothing or placeholder
  if (!bundleId || loading) {
    return null;
  }

  // If we have an icon URL, show the icon
  if (iconUrl && !error) {
    return (
      <img
        src={iconUrl}
        alt={appName || "App icon"}
        title={appName}
        className={`appIcon ${className}`}
        style={{ width: size, height: size }}
        onError={() => setError(true)}
      />
    );
  }

  // Fallback: show first letter of app name
  if (appName) {
    return (
      <div
        className={`appIconFallback ${className}`}
        title={appName}
        style={{ width: size, height: size, fontSize: size * 0.5 }}
      >
        {appName.charAt(0).toUpperCase()}
      </div>
    );
  }

  return null;
}
