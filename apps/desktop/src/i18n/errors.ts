import { AppError } from "../ipc";
import { m } from "../paraglide/messages.js";

const byCode: Record<string, () => string> = {
  bad_passphrase: m.error_bad_passphrase,
  weak_passphrase: m.error_weak_passphrase,
  invalid: m.error_invalid,
  locked: m.error_locked,
  corrupt_key_file: m.error_corrupt_key_file,
  not_found: m.error_not_found,
  remote_unauthorized: m.error_remote_unauthorized,
  remote_rate_limited: m.error_remote_rate_limited,
  remote_network: m.error_remote_network,
  remote_http: m.error_remote_http,
  remote_decode: m.error_remote_http,
  remote_timeout: m.error_remote_timeout,
  remote_cancelled: m.error_remote_cancelled,
  remote_not_linked: m.error_remote_not_linked,
};

/** Message d'un code d'erreur enregistré (dernière synchro, par exemple). */
export function codeMessage(code: string): string {
  return (byCode[code] ?? m.error_generic)();
}

export function errorMessage(e: unknown): string {
  if (e instanceof AppError && e.code === "invalid" && e.field === "backup") {
    return m.error_invalid_backup();
  }
  if (e instanceof AppError && e.code === "invalid") {
    if (e.field === "phone") return m.error_invalid_phone();
    if (e.field === "avatar") return m.member_avatar_error();
    if (e.field === "roles") return m.error_invalid_roles();
    if (e.field === "description") return m.error_invalid_description();
    if (e.field === "contacts") return m.error_invalid_contacts();
    if (e.field === "warningSigns" || e.field === "coping" || e.field === "reasons") {
      return m.error_invalid_plan();
    }
  }
  if (e instanceof AppError) return (byCode[e.code] ?? m.error_generic)();
  console.error(e);
  return m.error_generic();
}
