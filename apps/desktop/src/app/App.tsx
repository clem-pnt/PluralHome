import { useMutation, useQuery } from "@tanstack/react-query";
import { useCallback } from "react";
import { UpdateBanner } from "../features/updates/UpdateBanner";
import { SetupScreen } from "../features/vault/SetupScreen";
import { UnlockScreen } from "../features/vault/UnlockScreen";
import { api } from "../ipc";
import { keys, queryClient } from "./query";
import { Shell } from "./Shell";
import { useIdleLock } from "./useIdleLock";

const IDLE_LOCK_MINUTES = 10;

export function App() {
  const status = useQuery({ queryKey: keys.vaultStatus, queryFn: api.vault.status });

  const { mutate: lock } = useMutation({
    mutationFn: api.vault.lock,
    // Vider le cache efface aussi de la mémoire du front les données déchiffrées.
    onSuccess: () => queryClient.clear(),
  });
  const lockNow = useCallback(() => {
    if (queryClient.getQueryData(keys.vaultStatus) === "unlocked") lock();
  }, [lock]);
  useIdleLock(IDLE_LOCK_MINUTES, lockNow);

  return (
    <>
      <UpdateBanner />
      {status.data === "uninitialized" && <SetupScreen />}
      {status.data === "locked" && <UnlockScreen />}
      {status.data === "unlocked" && <Shell onLock={lockNow} />}
    </>
  );
}
