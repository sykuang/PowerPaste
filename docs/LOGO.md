# PowerPaste Logo

## Design

The PowerPaste logo combines two key concepts:
- **Clipboard**: Representing clipboard management functionality
- **Lightning Bolt**: Representing speed and power (the "Power" in PowerPaste)

### Colors
The logo uses the PowerPaste brand colors from the design system:
- **Primary**: `#0D9488` (Teal)
- **Secondary**: `#14B8A6` (Light Teal)
- **Accent**: `#F97316` (Orange) - used for the lightning bolt
- **Background**: `#F0FDFA` (Very light teal/white)
- **Dark**: `#134E4A` (Dark teal)

## Files

### Source Files
- **[src/assets/logo.svg](../src/assets/logo.svg)**: Master SVG logo file
- **[src/components/PowerPasteLogo.tsx](../src/components/PowerPasteLogo.tsx)**: React component for using the logo in the UI

### Generated Icons
All platform-specific icons are generated from the master SVG:
- **src-tauri/icons/icon.png** (512x512): Main app icon
- **src-tauri/icons/icon.icns**: macOS app icon bundle
- **src-tauri/icons/icon.ico**: Windows app icon
- **src-tauri/icons/*.png**: Various sizes for different platforms

## Usage

### In React Components

```tsx
import { PowerPasteLogo } from './components/PowerPasteLogo';

// Default (colorful, 64px)
<PowerPasteLogo />

// Custom size
<PowerPasteLogo size={128} />

// Monochrome (uses currentColor)
<PowerPasteLogo colorful={false} />

// With custom classes
<PowerPasteLogo className="my-logo" classNameBolt="animate-pulse" />
```

### Props
- `size`: Number (default: 64) - Size in pixels
- `colorful`: Boolean (default: true) - Whether to use brand colors or currentColor
- `className`: String - Additional CSS class for the SVG
- `classNameBolt`: String - Additional CSS class for the lightning bolt path

## Regenerating Icons

If you modify the logo SVG, regenerate all platform icons:

```bash
./scripts/generate-icons.sh
```

This requires ImageMagick to be installed:
```bash
brew install imagemagick
```

The script generates:
- PNG icons in various sizes
- Windows .ico file with multiple resolutions
- macOS .icns file with all required sizes
- Windows Store logo variants

## Design Notes

- The logo uses a glass/modern aesthetic consistent with the app's design system
- The lightning bolt is positioned on the "paper" to represent powerful clipboard operations
- The clip mechanism at the top reinforces the clipboard metaphor
- Retina-ready SVG scales perfectly at any size
