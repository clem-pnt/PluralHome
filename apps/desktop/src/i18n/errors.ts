import { AppError } from "../ipc";
import { m } from "../paraglide/messages.js";

const byCode: Record<string, () => string> = {
  bad_passphrase: m.error_bad_passphrase,
  weak_passphrase: m.error_weak_passphrase,
  invalid: m.error_invalid,
  locked: m.error_locked,
  corrupt_key_file: m.error_corrupt_key_file,
  key_missing: m.error_key_missing,
  not_found: m.error_not_found,
  remote_unauthorized: m.error_remote_unauthorized,
  remote_rate_limited: m.error_remote_rate_limited,
  remote_network: m.error_remote_network,
  remote_http: m.error_remote_http,
  remote_decode: m.error_remote_http,
  remote_timeout: m.error_remote_timeout,
  remote_cancelled: m.error_remote_cancelled,
  remote_not_linked: m.error_remote_not_linked,
  remote_conflict: m.error_remote_conflict,
  remote_quota: m.error_remote_quota,
  friend_self: m.error_friend_self,
  friend_already: m.error_friend_already,
  friend_blocked: m.error_friend_blocked,
  friend_inbox_full: m.error_friend_inbox_full,
  friend_too_many_requests: m.error_friend_too_many_requests,
  friend_too_many_friends: m.error_friend_too_many_friends,
  friend_too_many_invites: m.error_friend_too_many_invites,
  friend_invite_invalid: m.error_friend_invite_invalid,
  friend_request_gone: m.error_friend_request_gone,
  friend_gone: m.error_friend_gone,
  friend_mailbox_full: m.error_friend_mailbox_full,
  friend_identity_conflict: m.error_friend_identity_conflict,
  friend_no_identity: m.error_friend_no_identity,
};

/** Message d'un code d'erreur enregistré (dernière synchro, par exemple). */
export function codeMessage(code: string): string {
  return (byCode[code] ?? m.error_generic)();
}

export function errorMessage(e: unknown): string {
  if (e instanceof AppError && e.code === "invalid") {
    if (e.field === "recovery_key") return m.cloud_recovery_invalid();
    if (e.field === "phone") return m.error_invalid_phone();
    if (e.field === "fronter") return m.front_need_fronter();
    if (e.field === "avatar") return m.member_avatar_error();
    if (e.field === "roles") return m.error_invalid_roles();
    if (e.field === "folder") return m.error_invalid_folder();
    if (e.field === "description") return m.error_invalid_description();
    if (e.field === "title") return m.error_invalid_title();
    if (e.field === "contacts") return m.error_invalid_contacts();
    if (e.field === "welcome") return m.error_invalid_welcome();
    if (e.field === "basics") return m.error_invalid_basics();
    if (e.field === "safe") return m.error_invalid_safe();
    if (e.field === "steps") return m.error_invalid_steps();
    if (e.field === "section_title") return m.error_invalid_section_title();
    if (e.field === "sections") return m.error_invalid_sections();
    if (e.field === "channel") return m.error_invalid_channel();
    if (e.field === "channels") return m.error_invalid_channels();
    if (e.field === "question") return m.error_invalid_question();
    if (e.field === "options") return m.error_invalid_options();
    if (e.field === "closed") return m.error_poll_closed();
    if (e.field === "friend_closed") return m.error_friend_closed();
    if (e.field === "friend") return m.error_invalid_friend();
    if (e.field === "recipients") return m.error_invalid_recipients();
    if (e.field === "members") return m.error_invalid_members();
    if (e.field === "nickname") return m.error_invalid_nickname();
    if (e.field === "privacy") return m.error_invalid_privacy();
    if (e.field === "voters") return m.error_invalid_voters();
    if (e.field === "warningSigns" || e.field === "coping" || e.field === "reasons") {
      return m.error_invalid_plan();
    }
  }
  if (e instanceof AppError) return (byCode[e.code] ?? m.error_generic)();
  console.error(e);
  return m.error_generic();
}
