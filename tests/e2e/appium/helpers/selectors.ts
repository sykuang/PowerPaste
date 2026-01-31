/**
 * Cross-platform element selectors for Appium tests.
 *
 * Provides a unified interface for locating elements across
 * macOS (Mac2 driver) and Windows (Windows driver).
 */

type SelectorMap = {
  mac: string;
  windows: string;
};

/**
 * Element selectors organized by UI component.
 */
export const selectors = {
  // Main window
  mainWindow: {
    mac: 'XCUIElementTypeWindow',
    windows: '//Window',
  },

  // Application element
  application: {
    mac: 'XCUIElementTypeApplication',
    windows: '//Application',
  },

  // Buttons (by accessibility ID)
  settingsButton: {
    mac: '~settings-button',
    windows: '~settings-button',
  },
  closeButton: {
    mac: '~close-button',
    windows: '~close-button',
  },

  // Clipboard list
  clipboardList: {
    mac: '-ios class chain:**/XCUIElementTypeScrollView',
    windows: '//ScrollViewer',
  },
  clipboardItem: {
    mac: '-ios class chain:**/XCUIElementTypeGroup[`name BEGINSWITH "clipboard-item-"`]',
    windows: '//Group[starts-with(@Name, "clipboard-item-")]',
  },

  // Modal dialogs
  settingsModal: {
    mac: '~settings-modal',
    windows: '~settings-modal',
  },
  permissionsModal: {
    mac: '~permissions-modal',
    windows: '~permissions-modal',
  },

  // Permission buttons (macOS specific)
  openAccessibilityButton: {
    mac: '~open-accessibility-settings',
    windows: '~open-accessibility-settings',
  },
  openAutomationButton: {
    mac: '~open-automation-settings',
    windows: '~open-automation-settings',
  },

  // Search input
  searchInput: {
    mac: '~search-input',
    windows: '~search-input',
  },

  // Tabs
  tabBar: {
    mac: '~tab-bar',
    windows: '~tab-bar',
  },
  tabItem: {
    mac: '-ios class chain:**/XCUIElementTypeButton[`name BEGINSWITH "tab-"`]',
    windows: '//Button[starts-with(@Name, "tab-")]',
  },

  // System dialogs (for permission testing)
  systemDialog: {
    mac: 'XCUIElementTypeSheet',
    windows: '//Window[@Name="User Account Control"]',
  },
  allowButton: {
    mac: '-ios predicate string:label == "Allow" OR label == "OK"',
    windows: '//Button[@Name="Allow" or @Name="Yes"]',
  },
  denyButton: {
    mac: '-ios predicate string:label == "Deny" OR label == "Don\'t Allow"',
    windows: '//Button[@Name="Deny" or @Name="No"]',
  },
} as const;

/**
 * Get the appropriate selector for the current platform.
 */
export function getSelector(name: keyof typeof selectors): string {
  const platform = process.platform === 'darwin' ? 'mac' : 'windows';
  return selectors[name][platform];
}

/**
 * Get selector for a specific platform.
 */
export function getSelectorForPlatform(
  name: keyof typeof selectors,
  platform: 'mac' | 'windows'
): string {
  return selectors[name][platform];
}

/**
 * Create a dynamic accessibility ID selector.
 */
export function accessibilityId(id: string): string {
  return `~${id}`;
}

/**
 * Create a dynamic XPath selector based on platform.
 */
export function dynamicSelector(
  macSelector: string,
  windowsSelector: string
): string {
  return process.platform === 'darwin' ? macSelector : windowsSelector;
}

/**
 * macOS-specific: Create a predicate string selector.
 */
export function macPredicate(predicate: string): string {
  return `-ios predicate string:${predicate}`;
}

/**
 * macOS-specific: Create a class chain selector.
 */
export function macClassChain(chain: string): string {
  return `-ios class chain:${chain}`;
}
