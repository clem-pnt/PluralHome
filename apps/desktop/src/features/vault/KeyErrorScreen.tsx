import { DialogTrigger } from "react-aria-components";
import { errorMessage } from "../../i18n/errors";
import { AppError } from "../../ipc";
import { m } from "../../paraglide/messages.js";
import { Button } from "../../ui/Button";
import { RestoreDialog } from "../backup/RestoreDialog";
import { AuthLayout } from "./AuthLayout";

/** Le coffre existe mais ne s'ouvre pas (clé disparue du trousseau, par exemple). */
export function KeyErrorScreen({ error, onRetry }: { error: unknown; onRetry: () => void }) {
  const missing = error instanceof AppError && error.code === "key_missing";
  return (
    <AuthLayout title={m.keyerror_title()} lead={missing ? m.keyerror_lead() : errorMessage(error)}>
      <div className="auth__form">
        {missing && <p className="note">{m.keyerror_note()}</p>}
        <Button block onPress={onRetry}>
          {m.keyerror_retry()}
        </Button>
      </div>
      <DialogTrigger>
        <Button variant="ghost" block>
          {m.backup_restore()}
        </Button>
        <RestoreDialog />
      </DialogTrigger>
    </AuthLayout>
  );
}
