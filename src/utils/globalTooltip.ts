import { ref } from "vue";

export interface GlobalTooltipState {
  text: string;
  x: number;
  y: number;
  placement: "top" | "bottom";
  align: "left" | "center" | "right";
}

export const globalTooltipState = ref<GlobalTooltipState | null>(null);

const HOVER_DELAY_MS = 380;

let activeTarget: Element | null = null;
let hoverTimer: ReturnType<typeof setTimeout> | null = null;
let installed = false;

function clearHoverTimer() {
  if (hoverTimer !== null) {
    clearTimeout(hoverTimer);
    hoverTimer = null;
  }
}

function extractTooltipText(el: Element): string {
  const nativeTitle = el.getAttribute("title");
  if (nativeTitle !== null) {
    if (nativeTitle.trim()) {
      el.setAttribute("data-tooltip", nativeTitle);
    } else {
      el.removeAttribute("data-tooltip");
    }
    el.removeAttribute("title");
  }
  return el.getAttribute("data-tooltip")?.trim() ?? "";
}

function findTooltipTarget(start: EventTarget | null): { el: Element; text: string } | null {
  let cur = start instanceof Element ? start : null;
  while (cur) {
    if (cur.tagName.toLowerCase() !== "title") {
      if (cur.hasAttribute("title") || cur.hasAttribute("data-tooltip")) {
        const text = extractTooltipText(cur);
        if (text) {
          return { el: cur, text };
        }
      }
    }
    cur = cur.parentElement;
  }
  return null;
}

function showForElement(el: Element, text: string) {
  if (!el.isConnected) return;
  const rect = el.getBoundingClientRect();
  if (rect.width === 0 && rect.height === 0) return;

  const placement: "top" | "bottom" = rect.top < 48 ? "bottom" : "top";
  const y = placement === "top" ? rect.top - 8 : rect.bottom + 8;

  let align: "left" | "center" | "right" = "center";
  let x = rect.left + rect.width / 2;

  if (rect.left < 120) {
    align = "left";
    x = Math.max(8, rect.left);
  } else if (rect.right > window.innerWidth - 120) {
    align = "right";
    x = Math.min(window.innerWidth - 8, rect.right);
  }

  globalTooltipState.value = { text, x, y, placement, align };
}

function hide() {
  clearHoverTimer();
  activeTarget = null;
  globalTooltipState.value = null;
}

function onMouseOver(ev: MouseEvent) {
  const found = findTooltipTarget(ev.target);
  if (!found) {
    if (activeTarget) hide();
    return;
  }
  if (activeTarget === found.el) {
    return;
  }
  clearHoverTimer();
  globalTooltipState.value = null;
  activeTarget = found.el;
  const targetEl = found.el;
  const targetText = found.text;
  hoverTimer = setTimeout(() => {
    hoverTimer = null;
    if (activeTarget === targetEl) {
      showForElement(targetEl, targetText);
    }
  }, HOVER_DELAY_MS);
}

function onMouseOut(ev: MouseEvent) {
  if (!activeTarget) return;
  const related = ev.relatedTarget instanceof Node ? ev.relatedTarget : null;
  if (related && activeTarget.contains(related)) return;
  const nextFound = findTooltipTarget(related);
  if (nextFound && nextFound.el === activeTarget) return;
  hide();
}

export function installGlobalTooltip(): () => void {
  if (installed || typeof document === "undefined") {
    return () => {};
  }
  installed = true;

  const onPointerOrKey = () => hide();
  const onScroll = () => hide();

  document.addEventListener("mouseover", onMouseOver, true);
  document.addEventListener("mouseout", onMouseOut, true);
  document.addEventListener("mousedown", onPointerOrKey, true);
  document.addEventListener("keydown", onPointerOrKey, true);
  document.addEventListener("scroll", onScroll, true);
  window.addEventListener("blur", onPointerOrKey);

  return () => {
    installed = false;
    document.removeEventListener("mouseover", onMouseOver, true);
    document.removeEventListener("mouseout", onMouseOut, true);
    document.removeEventListener("mousedown", onPointerOrKey, true);
    document.removeEventListener("keydown", onPointerOrKey, true);
    document.removeEventListener("scroll", onScroll, true);
    window.removeEventListener("blur", onPointerOrKey);
    hide();
  };
}
