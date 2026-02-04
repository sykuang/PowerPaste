import { useState, useEffect } from "react";

interface RGB {
  r: number;
  g: number;
  b: number;
}

// Cache dominant colors by image URL
const colorCache = new Map<string, string>();

/**
 * Extracts the dominant color from an image URL using canvas sampling.
 * Returns a hex color string or null if extraction fails.
 */
export function useDominantColor(imageUrl: string | null): string | null {
  const [color, setColor] = useState<string | null>(() => {
    if (imageUrl && colorCache.has(imageUrl)) {
      return colorCache.get(imageUrl) || null;
    }
    return null;
  });

  useEffect(() => {
    if (!imageUrl) {
      setColor(null);
      return;
    }

    let cancelled = false;

    // Check cache first
    if (colorCache.has(imageUrl)) {
      if (!cancelled) {
        setColor(colorCache.get(imageUrl) || null);
      }
      return;
    }

    const img = new Image();
    img.crossOrigin = "anonymous";
    
    img.onload = () => {
      if (cancelled) return;
      try {
        const canvas = document.createElement("canvas");
        const ctx = canvas.getContext("2d");
        if (!ctx) {
          return;
        }

        // Sample a small version for performance
        const sampleSize = 32;
        canvas.width = sampleSize;
        canvas.height = sampleSize;
        
        ctx.drawImage(img, 0, 0, sampleSize, sampleSize);
        const imageData = ctx.getImageData(0, 0, sampleSize, sampleSize);
        const pixels = imageData.data;

        // Collect colors, excluding very dark, very light, and transparent pixels
        const colors: RGB[] = [];
        for (let i = 0; i < pixels.length; i += 4) {
          const r = pixels[i];
          const g = pixels[i + 1];
          const b = pixels[i + 2];
          const a = pixels[i + 3];

          // Skip transparent pixels
          if (a < 128) continue;

          // Skip very dark (near black) pixels
          if (r < 30 && g < 30 && b < 30) continue;

          // Skip very light (near white) pixels
          if (r > 225 && g > 225 && b > 225) continue;

          // Skip gray pixels (low saturation)
          const max = Math.max(r, g, b);
          const min = Math.min(r, g, b);
          const saturation = max === 0 ? 0 : (max - min) / max;
          if (saturation < 0.15) continue;

          colors.push({ r, g, b });
        }

        if (colors.length === 0) {
          // Fallback: just use any non-transparent pixel
          for (let i = 0; i < pixels.length; i += 4) {
            if (pixels[i + 3] >= 128) {
              colors.push({ r: pixels[i], g: pixels[i + 1], b: pixels[i + 2] });
              break;
            }
          }
        }

        if (colors.length === 0) {
          return;
        }

        // Average the colors (simple approach, could use k-means for better results)
        const avg = colors.reduce(
          (acc, c) => ({ r: acc.r + c.r, g: acc.g + c.g, b: acc.b + c.b }),
          { r: 0, g: 0, b: 0 }
        );
        avg.r = Math.round(avg.r / colors.length);
        avg.g = Math.round(avg.g / colors.length);
        avg.b = Math.round(avg.b / colors.length);

        // Boost saturation for more vibrant title colors
        const boosted = boostSaturation(avg, 1.3);

        const hex = rgbToHex(boosted.r, boosted.g, boosted.b);
        colorCache.set(imageUrl, hex);
        if (!cancelled) {
          setColor(hex);
        }
      } catch {
        // Canvas might fail due to CORS or other issues
      }
    };

    img.onerror = () => {
      // Image failed to load
    };

    img.src = imageUrl;

    return () => {
      cancelled = true;
    };
  }, [imageUrl]);

  return color;
}

function rgbToHex(r: number, g: number, b: number): string {
  return "#" + [r, g, b].map((x) => x.toString(16).padStart(2, "0")).join("");
}

function boostSaturation(rgb: RGB, factor: number): RGB {
  // Convert to HSL, boost saturation, convert back
  const r = rgb.r / 255;
  const g = rgb.g / 255;
  const b = rgb.b / 255;

  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  
  let h = 0;
  let s = 0;

  if (max !== min) {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    
    switch (max) {
      case r:
        h = ((g - b) / d + (g < b ? 6 : 0)) / 6;
        break;
      case g:
        h = ((b - r) / d + 2) / 6;
        break;
      case b:
        h = ((r - g) / d + 4) / 6;
        break;
    }
  }

  // Boost saturation
  s = Math.min(1, s * factor);

  // Convert back to RGB
  if (s === 0) {
    const gray = Math.round(l * 255);
    return { r: gray, g: gray, b: gray };
  }

  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;

  const hueToRgb = (t: number): number => {
    if (t < 0) t += 1;
    if (t > 1) t -= 1;
    if (t < 1 / 6) return p + (q - p) * 6 * t;
    if (t < 1 / 2) return q;
    if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
    return p;
  };

  return {
    r: Math.round(hueToRgb(h + 1 / 3) * 255),
    g: Math.round(hueToRgb(h) * 255),
    b: Math.round(hueToRgb(h - 1 / 3) * 255),
  };
}
