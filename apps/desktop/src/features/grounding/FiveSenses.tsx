import { useState } from "react";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";

const STEPS = [
  { count: 5, text: m.senses_see },
  { count: 4, text: m.senses_touch },
  { count: 3, text: m.senses_hear },
  { count: 2, text: m.senses_smell },
  { count: 1, text: m.senses_taste },
] as const;

/** Exercice 5-4-3-2-1 : ramener l'attention vers ce que perçoivent les sens, une étape à la fois. */
export function FiveSenses() {
  const [step, setStep] = useState(0);
  const done = step >= STEPS.length;
  const current = STEPS[Math.min(step, STEPS.length - 1)] ?? STEPS[0];

  return (
    <div className="senses">
      {done ? (
        <p className="senses__text" aria-live="polite">
          {m.senses_done()}
        </p>
      ) : (
        <>
          <p className="senses__count" aria-hidden>
            {current.count}
          </p>
          <p className="senses__text" aria-live="polite">
            {current.text()}
          </p>
          <ol className="senses__dots" aria-label={m.senses_progress({ step: step + 1 })}>
            {STEPS.map((s, i) => (
              <li
                key={s.count}
                data-active={i === step || undefined}
                data-past={i < step || undefined}
              />
            ))}
          </ol>
        </>
      )}
      <div className="panel__actions">
        {step > 0 && !done && (
          <Button variant="ghost" onPress={() => setStep((s) => s - 1)}>
            {m.senses_back()}
          </Button>
        )}
        {done ? (
          <Button variant="ghost" onPress={() => setStep(0)}>
            {m.senses_again()}
          </Button>
        ) : (
          <Button onPress={() => setStep((s) => s + 1)}>{m.senses_next()}</Button>
        )}
      </div>
    </div>
  );
}
