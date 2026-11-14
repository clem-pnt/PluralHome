import { useQuery } from "@tanstack/react-query";
import { UpdateBanner } from "../features/updates/UpdateBanner";
import { KeyErrorScreen } from "../features/vault/KeyErrorScreen";
import { SetupScreen } from "../features/vault/SetupScreen";
import { UpgradeScreen } from "../features/vault/UpgradeScreen";
import { api } from "../ipc";
import { keys } from "./query";
import { Shell } from "./Shell";

export function App() {
  // Le coffre s'ouvre tout seul : sa clé est rangée sur l'appareil.
  const status = useQuery({ queryKey: keys.vaultStatus, queryFn: api.vault.open });

  return (
    <>
      <UpdateBanner />
      {status.isError && (
        <KeyErrorScreen error={status.error} onRetry={() => void status.refetch()} />
      )}
      {status.data === "uninitialized" && <SetupScreen />}
      {status.data === "legacy" && <UpgradeScreen />}
      {status.data === "unlocked" && <Shell />}
    </>
  );
}
