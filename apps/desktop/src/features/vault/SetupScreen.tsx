import { useMutation } from "@tanstack/react-query";
import { useState } from "react";
import { Button as AriaButton } from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { api, type SystemKind } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { AccountChoice } from "./AccountChoice";
import { AuthLayout } from "./AuthLayout";

/**
 * Première ouverture, en deux temps : singlet ou système, puis compte Aragonite
 * ou invité. Le coffre est créé dès le premier choix (la connexion en a besoin) ;
 * l'application ne s'ouvre qu'après le second.
 */
export function SetupScreen() {
  const [created, setCreated] = useState(false);
  const [step, setStep] = useState<"kind" | "account">("kind");

  const choose = useMutation({
    mutationFn: async (kind: SystemKind) => {
      if (!created) {
        await api.vault.create();
        setCreated(true);
      }
      return api.system.setKind(kind);
    },
    onSuccess: (profile) => {
      queryClient.setQueryData(keys.system, profile);
      setStep("account");
    },
  });

  const enter = () => queryClient.setQueryData(keys.vaultStatus, "unlocked");

  if (step === "kind") {
    return (
      <AuthLayout title={m.setup_title()} lead={m.setup_lead()}>
        <div className="auth__form">
          <p className="auth__question">{m.setup_kind_question()}</p>
          <div className="auth__options">
            <AriaButton
              className="auth__option"
              isDisabled={choose.isPending}
              onPress={() => choose.mutate("system")}
            >
              <strong>{m.setup_kind_system()}</strong>
              <span>{m.setup_kind_system_hint()}</span>
            </AriaButton>
            <AriaButton
              className="auth__option"
              isDisabled={choose.isPending}
              onPress={() => choose.mutate("singlet")}
            >
              <strong>{m.setup_kind_singlet()}</strong>
              <span>{m.setup_kind_singlet_hint()}</span>
            </AriaButton>
          </div>
          {choose.isPending && (
            <p className="muted" role="status">
              {m.working()}
            </p>
          )}
          {choose.isError && (
            <p className="alert" role="alert">
              {errorMessage(choose.error)}
            </p>
          )}
          <p className="note">{m.setup_note()}</p>
        </div>
      </AuthLayout>
    );
  }

  return (
    <AuthLayout title={m.setup_account_title()} lead={m.setup_account_lead()}>
      <AccountChoice onEnter={enter} onBack={() => setStep("kind")} />
    </AuthLayout>
  );
}
