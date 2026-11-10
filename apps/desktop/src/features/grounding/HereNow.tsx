import { useEffect, useState } from "react";
import { m } from "../../paraglide/messages.js";
import { getLocale } from "../../paraglide/runtime.js";

const date = new Intl.DateTimeFormat(getLocale(), {
  weekday: "long",
  day: "numeric",
  month: "long",
  year: "numeric",
});
const time = new Intl.DateTimeFormat(getLocale(), { hour: "2-digit", minute: "2-digit" });

/** Repères d'orientation : utile quand on arrive au front sans savoir quel jour on est. */
export function HereNow() {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 15_000);
    return () => window.clearInterval(id);
  }, []);

  return (
    <div className="herenow">
      <p className="herenow__date">{date.format(now)}</p>
      <p className="herenow__time">{time.format(now)}</p>
      <ul className="herenow__lines">
        <li>{m.herenow_1()}</li>
        <li>{m.herenow_2()}</li>
        <li>{m.herenow_3()}</li>
      </ul>
    </div>
  );
}
