import { useEffect, useState } from "react";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";

/** Respiration au carré : 4 temps par phase, le rythme le plus simple à suivre en crise. */
const PHASE_SECONDS = 4;
const PHASES = [
  { label: m.breath_in, scale: 1 },
  { label: m.breath_hold, scale: 1 },
  { label: m.breath_out, scale: 0.6 },
  { label: m.breath_hold, scale: 0.6 },
] as const;

export function Breathing() {
  const [running, setRunning] = useState(false);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    if (!running) return;
    const id = window.setInterval(() => setTick((t) => t + 1), 1000);
    return () => window.clearInterval(id);
  }, [running]);

  const phase = PHASES[Math.floor(tick / PHASE_SECONDS) % PHASES.length] ?? PHASES[0];
  const left = PHASE_SECONDS - (tick % PHASE_SECONDS);

  return (
    <div className="breathing">
      <div
        className="breathing__circle"
        data-running={running || undefined}
        style={{ "--scale": running ? phase.scale : 0.6 } as React.CSSProperties}
        aria-hidden
      />
      <p className="breathing__phase" aria-live="polite">
        {running ? phase.label() : m.breath_ready()}
      </p>
      <p className="breathing__count" aria-hidden>
        {running ? left : ""}
      </p>
      <Button
        variant={running ? "ghost" : "primary"}
        onPress={() => {
          setTick(0);
          setRunning((r) => !r);
        }}
      >
        {running ? m.breath_stop() : m.breath_start()}
      </Button>
    </div>
  );
}
