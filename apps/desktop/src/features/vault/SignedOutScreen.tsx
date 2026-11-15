import { m } from "../../paraglide/messages.js";
import { AccountChoice } from "./AccountChoice";
import { AuthLayout } from "./AuthLayout";

/** Après une déconnexion d'Aragonite : on sort de l'application, les données restent sur l'appareil. */
export function SignedOutScreen({ onEnter }: { onEnter: () => void }) {
  return (
    <AuthLayout title={m.signed_out_title()} lead={m.signed_out_lead()}>
      <AccountChoice onEnter={onEnter} />
    </AuthLayout>
  );
}
