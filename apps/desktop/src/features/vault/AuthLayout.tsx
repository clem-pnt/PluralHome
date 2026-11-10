import type { ReactNode } from "react";
import { m } from "../../paraglide/messages.js";
import { Logo } from "../../ui/Logo";
import { GroundingDialog } from "../grounding/GroundingDialog";

export function AuthLayout({
  title,
  lead,
  children,
}: {
  title: string;
  lead: string;
  children: ReactNode;
}) {
  return (
    <div className="auth">
      <div className="auth__panel">
        <header className="auth__header">
          <Logo size={56} title={m.app_name()} />
          <h1>{title}</h1>
          <p>{lead}</p>
        </header>
        {children}
      </div>
      <GroundingDialog />
      <p className="auth__foot">{m.app_tagline()}</p>
    </div>
  );
}
