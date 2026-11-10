import { Tab, TabList, TabPanel, Tabs } from "react-aria-components";
import { m } from "../../paraglide/messages.js";
import { Breathing } from "./Breathing";
import { CrisisLines } from "./CrisisLines";
import { FiveSenses } from "./FiveSenses";
import { HereNow } from "./HereNow";
import { SafetyPlan } from "./SafetyPlan";

/**
 * Outils d'ancrage. `withPlan` : coffre ouvert, le plan de crise personnel est proposé.
 * Sans lui, tout reste utilisable coffre verrouillé : rien ici ne lit de donnée chiffrée.
 */
export function Grounding({ withPlan }: { withPlan: boolean }) {
  return (
    <Tabs className="grounding" defaultSelectedKey="breathe">
      <TabList className="tabs" aria-label={m.grounding_title()}>
        <Tab id="breathe" className="tab">
          {m.grounding_breathe()}
        </Tab>
        <Tab id="senses" className="tab">
          {m.grounding_senses()}
        </Tab>
        <Tab id="now" className="tab">
          {m.grounding_now()}
        </Tab>
        {withPlan && (
          <Tab id="plan" className="tab">
            {m.grounding_plan()}
          </Tab>
        )}
        <Tab id="help" className="tab">
          {m.grounding_help()}
        </Tab>
      </TabList>
      <TabPanel id="breathe" className="grounding__panel">
        <Breathing />
      </TabPanel>
      <TabPanel id="senses" className="grounding__panel">
        <FiveSenses />
      </TabPanel>
      <TabPanel id="now" className="grounding__panel">
        <HereNow />
      </TabPanel>
      {withPlan && (
        <TabPanel id="plan" className="grounding__panel">
          <SafetyPlan />
        </TabPanel>
      )}
      <TabPanel id="help" className="grounding__panel">
        <p className="muted">{m.lines_lead()}</p>
        <CrisisLines />
      </TabPanel>
    </Tabs>
  );
}
