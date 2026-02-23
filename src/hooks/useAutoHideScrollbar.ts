import { useEffect, type RefObject } from "react";

/**
 * Auto-hide scrollbar hook — macOS overlay style.
 *
 * Sets `data-scrolling="true"` on the element while the user is scrolling,
 * then removes it after `hideDelay` ms of inactivity. CSS rules in App.css
 * key off `[data-scrolling]` to show/hide the scrollbar thumb.
 *
 * @param ref   React ref to the scrollable container
 * @param hideDelay  ms to wait after last scroll event before hiding (default 1200)
 */
export function useAutoHideScrollbar(
  ref: RefObject<HTMLElement | null>,
  hideDelay = 1200,
): void {
  useEffect(() => {
    const el = ref.current;
    if (!el) return;

    let timer: ReturnType<typeof setTimeout> | null = null;

    const show = () => {
      if (timer) clearTimeout(timer);
      el.setAttribute("data-scrolling", "true");
      timer = setTimeout(() => {
        el.removeAttribute("data-scrolling");
      }, hideDelay);
    };

    el.addEventListener("scroll", show, { passive: true });

    return () => {
      el.removeEventListener("scroll", show);
      if (timer) clearTimeout(timer);
      el.removeAttribute("data-scrolling");
    };
  }, [ref, hideDelay]);
}
