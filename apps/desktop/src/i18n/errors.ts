import { AppError } from "../ipc";
import { m } from "../paraglide/messages.js";

const byCode: Record<string, () => string> = {
  bad_passphrase: m.error_bad_passphrase,
  weak_passphrase: m.error_weak_passphrase,
  invalid: m.error_invalid,
  locked: m.error_locked,
  corrupt_key_file: m.error_corrupt_key_file,
  not_found: m.error_not_found,
};

export function errorMessage(e: unknown): string {
  if (e instanceof AppError) return (byCode[e.code] ?? m.error_generic)();
  console.error(e);
  return m.error_generic();
}
