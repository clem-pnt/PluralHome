import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { Dialog, DialogTrigger, Form, Heading, Modal, ModalOverlay } from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { codeMessage, errorMessage } from "../../i18n/errors";
import { formatWhen } from "../../i18n/format";
import { AppError, api } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Avatar } from "../../ui/Avatar";
import { Button } from "../../ui/Button";
import { TextField } from "../../ui/TextField";

export function ConnectionsView() {
  return (
    <section className="view">
      <header className="view__header">
        <div>
          <h1>{m.connections_title()}</h1>
          <p className="view__lead">{m.connections_lead()}</p>
        </div>
      </header>
      <div className="connections">
        <AccountPanel />
        <PluralKitPanel />
      </div>
    </section>
  );
}

// ---------- compte Aragonite ----------

function AccountPanel() {
  const account = useQuery({ queryKey: keys.account, queryFn: api.account.get });
  const setAccount = (a: Awaited<ReturnType<typeof api.account.get>>) =>
    queryClient.setQueryData(keys.account, a);

  const login = useMutation({ mutationFn: api.account.login, onSuccess: setAccount });
  const refresh = useMutation({ mutationFn: api.account.refresh, onSuccess: setAccount });
  const logout = useMutation({ mutationFn: api.account.logout, onSuccess: () => setAccount(null) });
  // Une annulation voulue n'est pas une erreur à afficher.
  const failure = [login, refresh, logout].find(
    (x) => x.isError && !(x.error instanceof AppError && x.error.code === "remote_cancelled"),
  );
  const a = account.data;

  return (
    <div className="panel">
      <h2 className="panel__title">{m.account_title()}</h2>
      <p className="muted">{m.account_body()}</p>

      {a ? (
        <>
          <div className="identity">
            <Avatar name={a.displayName || "?"} color="var(--accent)" />
            <div className="identity__text">
              <strong>{m.account_signed_in({ name: a.displayName })}</strong>
              <span className="muted">
                {a.discord
                  ? m.account_discord({ username: a.discord.username })
                  : m.account_no_discord()}
              </span>
            </div>
          </div>
          <div className="panel__actions">
            <Button variant="ghost" isDisabled={refresh.isPending} onPress={() => refresh.mutate()}>
              {refresh.isPending ? m.working() : m.account_refresh()}
            </Button>
            <Button variant="ghost" isDisabled={logout.isPending} onPress={() => logout.mutate()}>
              {m.account_logout()}
            </Button>
          </div>
        </>
      ) : login.isPending ? (
        <div className="panel__actions">
          <p className="muted" role="status">
            {m.account_waiting()}
          </p>
          <Button variant="ghost" onPress={() => void api.account.cancelLogin()}>
            {m.cancel()}
          </Button>
        </div>
      ) : (
        <div>
          <Button isDisabled={!account.isSuccess} onPress={() => login.mutate()}>
            {m.account_login()}
          </Button>
        </div>
      )}

      {failure && (
        <p className="alert" role="alert">
          {errorMessage(failure.error)}
        </p>
      )}
    </div>
  );
}

// ---------- PluralKit ----------

function PluralKitPanel() {
  const status = useQuery({ queryKey: keys.pkStatus, queryFn: api.pluralkit.status });
  const s = status.data;

  const afterChange = () => {
    void queryClient.invalidateQueries({ queryKey: keys.pkStatus });
    void queryClient.invalidateQueries({ queryKey: keys.members });
    void queryClient.invalidateQueries({ queryKey: ["front"] });
  };
  const syncNow = useMutation({
    mutationFn: api.pluralkit.syncNow,
    onSuccess: (next) => {
      queryClient.setQueryData(keys.pkStatus, next);
      afterChange();
    },
  });

  return (
    <div className="panel">
      <h2 className="panel__title">{m.pk_title()}</h2>
      <p className="muted">{m.pk_body()}</p>

      {s?.enabled ? (
        <>
          <ul className="facts">
            <li>
              <strong>{m.pk_linked({ system: s.system ?? "" })}</strong>
            </li>
            <li className="muted">
              {s.lastSync ? m.pk_last_sync({ when: formatWhen(s.lastSync) }) : m.pk_never_synced()}
            </li>
            {(s.pending ?? 0) > 0 && (
              <li className="muted">{m.pk_pending({ count: s.pending ?? 0 })}</li>
            )}
            {s.lastError && (
              <li className="alert" role="alert">
                {m.pk_last_error({ error: codeMessage(s.lastError) })}
              </li>
            )}
          </ul>
          <p className="note">{m.pk_rules()}</p>
          <div className="panel__actions">
            <Button isDisabled={syncNow.isPending} onPress={() => syncNow.mutate()}>
              {syncNow.isPending ? m.working() : m.pk_sync_now()}
            </Button>
            <UnlinkButton onDone={afterChange} />
          </div>
        </>
      ) : (
        status.isSuccess && <LinkForm onDone={afterChange} />
      )}
    </div>
  );
}

function LinkForm({ onDone }: { onDone: () => void }) {
  const [token, setToken] = useState("");
  const link = useMutation({
    mutationFn: api.pluralkit.link,
    onSuccess: () => {
      setToken("");
      onDone();
    },
  });

  return (
    <Form
      className="auth__form"
      onSubmit={(e) => {
        e.preventDefault();
        link.mutate(token);
      }}
    >
      <p className="note">{m.pk_warning()}</p>
      <TextField
        label={m.pk_token()}
        hint={m.pk_token_hint()}
        type="password"
        autoComplete="off"
        spellCheck="false"
        isRequired
        value={token}
        onChange={setToken}
      />
      {link.isError && (
        <p className="alert" role="alert">
          {errorMessage(link.error)}
        </p>
      )}
      <div>
        <Button type="submit" isDisabled={link.isPending || token.trim().length === 0}>
          {link.isPending ? m.pk_linking() : m.pk_link()}
        </Button>
      </div>
    </Form>
  );
}

function UnlinkButton({ onDone }: { onDone: () => void }) {
  const unlink = useMutation({ mutationFn: api.pluralkit.unlink, onSuccess: onDone });
  return (
    <DialogTrigger>
      <Button variant="ghost">{m.pk_unlink()}</Button>
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal">
          <Dialog className="dialog" role="alertdialog">
            {({ close }) => (
              <>
                <Heading slot="title">{m.pk_unlink_title()}</Heading>
                <p className="muted">{m.pk_unlink_body()}</p>
                {unlink.isError && (
                  <p className="alert" role="alert">
                    {errorMessage(unlink.error)}
                  </p>
                )}
                <div className="dialog__actions">
                  <Button variant="ghost" onPress={close}>
                    {m.cancel()}
                  </Button>
                  <Button
                    variant="danger"
                    autoFocus
                    isDisabled={unlink.isPending}
                    onPress={() => unlink.mutate(undefined, { onSuccess: close })}
                  >
                    {m.pk_unlink()}
                  </Button>
                </div>
              </>
            )}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </DialogTrigger>
  );
}
