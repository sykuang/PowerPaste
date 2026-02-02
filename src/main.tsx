import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";

// Prevent any accidental link navigation in the app
// This stops the webview from opening new windows when text looks like a URL
document.addEventListener("click", (e) => {
  const target = e.target as HTMLElement;
  const anchor = target.closest("a");
  if (anchor && anchor.href) {
    // Only allow internal navigation (same origin)
    try {
      const url = new URL(anchor.href, window.location.origin);
      if (url.origin !== window.location.origin) {
        e.preventDefault();
        e.stopPropagation();
      }
    } catch {
      e.preventDefault();
      e.stopPropagation();
    }
  }
}, true);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
