import { useEffect } from "react";

const EVENTS = ["pointerdown", "keydown", "wheel"] as const;

/** Verrouille après `minutes` sans interaction : un écran laissé ouvert ne doit rien exposer. */
export function useIdleLock(minutes: number, onIdle: () => void) {
  useEffect(() => {
    let timer = window.setTimeout(onIdle, minutes * 60_000);
    const reset = () => {
      window.clearTimeout(timer);
      timer = window.setTimeout(onIdle, minutes * 60_000);
    };
    for (const e of EVENTS) window.addEventListener(e, reset, { passive: true });
    return () => {
      window.clearTimeout(timer);
      for (const e of EVENTS) window.removeEventListener(e, reset);
    };
  }, [minutes, onIdle]);
}
