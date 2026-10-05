import { useEffect, useState, useSyncExternalStore } from "react";

/**
 * Shows a busy indicator only when work is slow enough to notice, and keeps it
 * up long enough not to flicker: hidden for the first `delay` ms, then visible
 * for at least `minVisible` ms once shown.
 */
export function useDelayedBusy(busy: boolean, delay = 150, minVisible = 400): boolean {
  const [visible, setVisible] = useState(false);
  const [shownAt, setShownAt] = useState(0);
  useEffect(() => {
    if (busy && !visible) {
      const timer = setTimeout(() => {
        setVisible(true);
        setShownAt(Date.now());
      }, delay);
      return () => clearTimeout(timer);
    }
    if (!busy && visible) {
      const remaining = Math.max(0, minVisible - (Date.now() - shownAt));
      const timer = setTimeout(() => setVisible(false), remaining);
      return () => clearTimeout(timer);
    }
    return undefined;
  }, [busy, visible, delay, minVisible, shownAt]);
  return visible;
}

/* In-flight API requests, for the global progress bar. */
let inFlight = 0;
const listeners = new Set<() => void>();

function emit() {
  for (const listener of listeners) listener();
}

export function trackRequest<T>(promise: Promise<T>): Promise<T> {
  inFlight += 1;
  emit();
  return promise.finally(() => {
    inFlight -= 1;
    emit();
  });
}

export function useRequestsInFlight(): boolean {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => inFlight > 0,
    () => false,
  );
}
