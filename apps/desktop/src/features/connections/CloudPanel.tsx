import { useMutation, useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { Dialog, DialogTrigger, Form, Heading, Modal, ModalOverlay } from "react-aria-components";
import { keys, queryClient } from "../../app/query";
import { codeMessage, errorMessage } from "../../i18n/errors";
import { formatWhen } from "../../i18n/format";
import { AppError, api, type CloudStatus } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { TextField } from "../../ui/TextField";

const setStatus = (s: CloudStatus) => queryClient.setQueryData(keys.cloudStatus, s);
const refreshStatus = () => queryClient.invalidateQueries({ queryKey: keys.cloudStatus });
/** Un autre appareil vient d'apporter ses données : tout est à relire. */
const refreshAll = () => queryClient.invalidateQueries();

/** Synchro chiffrée de bout en bout entre les appareils du même compte Aragonite. */
export function CloudPanel() {
  const account = useQuery({ queryKey: keys.account, queryFn: api.account.get });
  const status = useQuery({ queryKey: keys.cloudStatus, queryFn: api.cloud.status });
  // La clé de secours n'est montrée qu'une fois, juste après l'activation.
  const [recovery, setRecovery] = useState<string | null>(null);
  const s = status.data;

  let body: React.ReactNode = null;
  if (recovery) body = <RecoveryKey value={recovery} onDone={() => setRecovery(null)} />;
  else if (!s || !account.isSuccess) body = null;
  else if (s.pairing) body = <PairingWait fingerprint={s.pairing} />;
  else if (s.enabled) body = <Enabled status={s} />;
  else if (!account.data) body = <p className="note">{m.cloud_need_account()}</p>;
  else body = <Setup onEnabled={setRecovery} />;

  return (
    <div className="panel">
      <h2 className="panel__title">{m.cloud_title()}</h2>
      <p className="muted">{m.cloud_body()}</p>
      {body}
    </div>
  );
}

// ---------- activation ----------

function Setup({ onEnabled }: { onEnabled: (recovery: string) => void }) {
  const [mode, setMode] = useState<"idle" | "new" | "join">("idle");
  const probe = useMutation({
    mutationFn: api.cloud.probe,
    onSuccess: (r) => setMode(r === "empty" ? "new" : "join"),
  });
  const enable = useMutation({
    mutationFn: api.cloud.enable,
    onSuccess: (key) => {
      onEnabled(key);
      void refreshStatus();
    },
    onError: (e) => {
      // Un autre appareil a activé la synchro entre-temps : il faut le rejoindre.
      if (e instanceof AppError && e.code === "remote_conflict") setMode("join");
    },
  });
  const pair = useMutation({ mutationFn: api.cloud.pairRequest, onSuccess: refreshStatus });
  const failure = [probe, enable, pair].find(
    (x) => x.isError && !(x.error instanceof AppError && x.error.code === "remote_conflict"),
  );

  return (
    <>
      {mode === "idle" && (
        <div>
          <Button isDisabled={probe.isPending} onPress={() => probe.mutate()}>
            {probe.isPending ? m.working() : m.cloud_enable()}
          </Button>
        </div>
      )}
      {mode === "new" && (
        <>
          <p className="note">{m.cloud_new_body()}</p>
          <div className="panel__actions">
            <Button isDisabled={enable.isPending} onPress={() => enable.mutate()}>
              {enable.isPending ? m.working() : m.cloud_new_confirm()}
            </Button>
            <Button variant="ghost" onPress={() => setMode("idle")}>
              {m.cancel()}
            </Button>
          </div>
        </>
      )}
      {mode === "join" && (
        <>
          <p className="note">{m.cloud_join_body()}</p>
          <div className="panel__actions">
            <Button isDisabled={pair.isPending} onPress={() => pair.mutate()}>
              {pair.isPending ? m.working() : m.cloud_join_pair()}
            </Button>
            <RecoveryJoin />
            <Button variant="ghost" onPress={() => setMode("idle")}>
              {m.cancel()}
            </Button>
          </div>
        </>
      )}
      {failure && (
        <p className="alert" role="alert">
          {errorMessage(failure.error)}
        </p>
      )}
    </>
  );
}

function RecoveryKey({ value, onDone }: { value: string; onDone: () => void }) {
  return (
    <div className="auth__form">
      <p className="note">{m.cloud_recovery_intro()}</p>
      <code className="recovery">{value}</code>
      <p className="muted">{m.cloud_recovery_advice()}</p>
      <div>
        <Button onPress={onDone}>{m.cloud_recovery_done()}</Button>
      </div>
    </div>
  );
}

function RecoveryJoin() {
  const [key, setKey] = useState("");
  const join = useMutation({
    mutationFn: api.cloud.joinRecovery,
    onSuccess: () => {
      setKey("");
      void refreshAll();
    },
  });
  const failure =
    join.error instanceof AppError && join.error.code === "bad_passphrase"
      ? m.cloud_recovery_wrong()
      : join.isError
        ? errorMessage(join.error)
        : null;

  return (
    <DialogTrigger>
      <Button variant="ghost">{m.cloud_join_recovery()}</Button>
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal">
          <Dialog className="dialog">
            {({ close }) => (
              <Form
                className="auth__form"
                onSubmit={(e) => {
                  e.preventDefault();
                  join.mutate(key, { onSuccess: close });
                }}
              >
                <Heading slot="title">{m.cloud_join_recovery()}</Heading>
                <p className="muted">{m.cloud_recovery_join_body()}</p>
                <TextField
                  label={m.cloud_recovery_label()}
                  autoComplete="off"
                  spellCheck="false"
                  isRequired
                  value={key}
                  onChange={setKey}
                />
                {failure && (
                  <p className="alert" role="alert">
                    {failure}
                  </p>
                )}
                <div className="dialog__actions">
                  <Button variant="ghost" onPress={close}>
                    {m.cancel()}
                  </Button>
                  <Button type="submit" isDisabled={join.isPending || key.trim().length === 0}>
                    {join.isPending ? m.working() : m.cloud_join_submit()}
                  </Button>
                </div>
              </Form>
            )}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </DialogTrigger>
  );
}

// ---------- appairage (côté nouvel appareil) ----------

function PairingWait({ fingerprint }: { fingerprint: string }) {
  const [gone, setGone] = useState(false);
  const cancel = useMutation({ mutationFn: api.cloud.pairCancel, onSuccess: refreshStatus });
  const poll = useQuery({
    queryKey: ["cloud", "pair-poll"],
    queryFn: api.cloud.pairPoll,
    refetchInterval: 3000,
    staleTime: 0,
    gcTime: 0,
  });

  useEffect(() => {
    if (poll.data === "joined") void refreshAll();
    if (poll.data === "gone") {
      setGone(true);
      void refreshStatus();
    }
  }, [poll.data]);

  if (gone) return <p className="alert">{m.cloud_pair_gone()}</p>;
  return (
    <div className="auth__form">
      <p className="note">{m.cloud_pair_wait()}</p>
      <code className="recovery">{fingerprint}</code>
      {poll.isError && (
        <p className="alert" role="alert">
          {errorMessage(poll.error)}
        </p>
      )}
      <div>
        <Button variant="ghost" isDisabled={cancel.isPending} onPress={() => cancel.mutate()}>
          {m.cancel()}
        </Button>
      </div>
    </div>
  );
}

// ---------- synchro active ----------

function Enabled({ status: s }: { status: CloudStatus }) {
  const syncNow = useMutation({
    mutationFn: api.cloud.syncNow,
    onSuccess: setStatus,
    onError: refreshStatus,
  });
  const disable = useMutation({ mutationFn: api.cloud.disable, onSuccess: refreshStatus });

  return (
    <>
      <ul className="facts">
        <li>
          <strong>{m.cloud_enabled()}</strong>
        </li>
        <li className="muted">
          {s.lastSync ? m.pk_last_sync({ when: formatWhen(s.lastSync) }) : m.pk_never_synced()}
        </li>
        {(s.pending ?? 0) > 0 && (
          <li className="muted">{m.cloud_pending({ count: s.pending ?? 0 })}</li>
        )}
        {s.lastError && (
          <li className="alert" role="alert">
            {m.pk_last_error({ error: codeMessage(s.lastError) })}
          </li>
        )}
      </ul>
      <Devices />
      <div className="panel__actions">
        <Button isDisabled={syncNow.isPending} onPress={() => syncNow.mutate()}>
          {syncNow.isPending ? m.working() : m.pk_sync_now()}
        </Button>
        <Button variant="ghost" isDisabled={disable.isPending} onPress={() => disable.mutate()}>
          {m.cloud_disable()}
        </Button>
        <DeleteButton />
      </div>
      {(syncNow.isError || disable.isError) && (
        <p className="alert" role="alert">
          {errorMessage(syncNow.error ?? disable.error)}
        </p>
      )}
    </>
  );
}

/** Appareils qui demandent à rejoindre : on compare l'empreinte avant d'accepter. */
function Devices() {
  const list = useQuery({
    queryKey: keys.cloudDevices,
    queryFn: api.cloud.pairRequests,
    refetchInterval: 5000,
    staleTime: 0,
  });
  const done = () => queryClient.invalidateQueries({ queryKey: keys.cloudDevices });
  const approve = useMutation({ mutationFn: api.cloud.pairApprove, onSettled: done });
  const reject = useMutation({ mutationFn: api.cloud.pairReject, onSettled: done });
  const devices = list.data ?? [];
  if (devices.length === 0 && !approve.isError && !reject.isError) return null;

  return (
    <div className="auth__form">
      {devices.map((d) => (
        <div key={d.id} className="note">
          <p>{m.cloud_device_asks()}</p>
          <code className="recovery">{d.fingerprint}</code>
          <p className="muted">{m.cloud_device_check()}</p>
          <div className="panel__actions">
            <Button isDisabled={approve.isPending} onPress={() => approve.mutate(d.id)}>
              {m.cloud_device_approve()}
            </Button>
            <Button
              variant="ghost"
              isDisabled={reject.isPending}
              onPress={() => reject.mutate(d.id)}
            >
              {m.cloud_device_reject()}
            </Button>
          </div>
        </div>
      ))}
      {(approve.isError || reject.isError) && (
        <p className="alert" role="alert">
          {errorMessage(approve.error ?? reject.error)}
        </p>
      )}
    </div>
  );
}

function DeleteButton() {
  const remove = useMutation({ mutationFn: api.cloud.remove, onSuccess: refreshStatus });
  return (
    <DialogTrigger>
      <Button variant="ghost">{m.cloud_delete()}</Button>
      <ModalOverlay className="overlay" isDismissable>
        <Modal className="modal">
          <Dialog className="dialog" role="alertdialog">
            {({ close }) => (
              <>
                <Heading slot="title">{m.cloud_delete_title()}</Heading>
                <p className="muted">{m.cloud_delete_body()}</p>
                {remove.isError && (
                  <p className="alert" role="alert">
                    {errorMessage(remove.error)}
                  </p>
                )}
                <div className="dialog__actions">
                  <Button variant="ghost" onPress={close}>
                    {m.cancel()}
                  </Button>
                  <Button
                    variant="danger"
                    autoFocus
                    isDisabled={remove.isPending}
                    onPress={() => remove.mutate(undefined, { onSuccess: close })}
                  >
                    {m.cloud_delete_confirm()}
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
