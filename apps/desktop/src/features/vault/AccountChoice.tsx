import { useMutation } from "@tanstack/react-query";
import { Button as AriaButton } from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { errorMessage } from "../../i18n/errors";
import { AppError, api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";

/**
 * Se connecter à Aragonite ou continuer en invité. Sert à la première ouverture
 * et après une déconnexion.
 */
export function AccountChoice({ onEnter, onBack }: { onEnter: () => void; onBack?: () => void }) {
  const login = useMutation({
    mutationFn: api.account.login,
    onSuccess: (a) => {
      queryClient.setQueryData(keys.account, a);
      queryClient.setQueryData(keys.signedOut, false);
      onEnter();
    },
  });
  const guest = useMutation({
    mutationFn: api.account.continueGuest,
    onSuccess: () => {
      queryClient.setQueryData(keys.signedOut, false);
      onEnter();
    },
  });
  const loginFailed =
    login.isError && !(login.error instanceof AppError && login.error.code === "remote_cancelled");

  return (
    <div className="auth__form">
      {login.isPending ? (
        <>
          <p className="muted" role="status">
            {m.account_waiting()}
          </p>
          <Button block variant="ghost" onPress={() => void api.account.cancelLogin()}>
            {m.cancel()}
          </Button>
        </>
      ) : (
        <>
          <Button block onPress={() => login.mutate()}>
            {m.setup_account_login()}
          </Button>
          <Button block variant="ghost" isDisabled={guest.isPending} onPress={() => guest.mutate()}>
            {m.setup_account_guest()}
          </Button>
        </>
      )}
      {loginFailed && (
        <p className="alert" role="alert">
          {errorMessage(login.error)}
        </p>
      )}
      {guest.isError && (
        <p className="alert" role="alert">
          {errorMessage(guest.error)}
        </p>
      )}
      <p className="note">{m.setup_account_note()}</p>
      {onBack && !login.isPending && (
        <AriaButton className="auth__back" onPress={onBack}>
          {m.setup_back()}
        </AriaButton>
      )}
    </div>
  );
}
