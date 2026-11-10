import { m } from "../../paraglide/messages.js";
import { Grounding } from "./Grounding";

export function GroundingView() {
  return (
    <section className="view">
      <header className="view__header">
        <div>
          <h1>{m.grounding_title()}</h1>
          <p className="view__lead">{m.grounding_lead()}</p>
        </div>
      </header>
      <Grounding withPlan />
    </section>
  );
}
