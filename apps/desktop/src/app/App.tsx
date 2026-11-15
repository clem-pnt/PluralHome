import { useQuery } from "@tanstack/react-query";
import { UpdateBanner } from "../features/updates/UpdateBanner";
import { KeyErrorScreen } from "../features/vault/KeyErrorScreen";
import { SetupScreen } from "../features/vault/SetupScreen";
import { SignedOutScreen } from "../features/vault/SignedOutScreen";
import { UpgradeScreen } from "../features/vault/UpgradeScreen";
import { api } from "../ipc";
import { keys, queryClient } from "./query";
import { Shell } from "./Shell";

export function App() {
  // Le coffre s'ouvre tout seul : sa clé est rangée sur l'appareil.
  const status = useQuery({ queryKey: keys.vaultStatus, queryFn: api.vault.open });
  const unlocked = status.data === "unlocked";
  // Déconnecté d'Aragonite : on sort de l'application jusqu'au choix connexion / invité.
  const signedOut = useQuery({
    queryKey: keys.signedOut,
    queryFn: api.account.signedOut,
    enabled: unlocked,
    // Une session refusée pendant une synchro se remarque sans relancer l'app.
    refetchInterval: 60_000,
  });

  return (
    <>
      <UpdateBanner />
      {status.isError && (
        <KeyErrorScreen error={status.error} onRetry={() => void status.refetch()} />
      )}
      {status.data === "uninitialized" && <SetupScreen />}
      {status.data === "legacy" && <UpgradeScreen />}
      {unlocked && signedOut.data === true && (
        <SignedOutScreen onEnter={() => queryClient.setQueryData(keys.signedOut, false)} />
      )}
      {unlocked && (signedOut.data === false || signedOut.isError) && <Shell />}
    </>
  );
}
