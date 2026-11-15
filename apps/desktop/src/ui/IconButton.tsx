import {
  Button as AriaButton,
  type ButtonProps as AriaButtonProps,
  Tooltip,
  TooltipTrigger,
} from "react-aria-components";
import { Icon, type IconName } from "./Icon";

export interface IconButtonProps extends Omit<AriaButtonProps, "className" | "children"> {
  icon: IconName;
  /** Nom lu par les lecteurs d'écran et affiché au survol. */
  label: string;
  tone?: "default" | "danger" | "accent";
  pressed?: boolean;
}

/** Bouton carré à icône, avec une infobulle qui en donne le nom. */
export function IconButton({ icon, label, tone = "default", pressed, ...props }: IconButtonProps) {
  return (
    <TooltipTrigger delay={400} closeDelay={0}>
      <AriaButton
        {...props}
        aria-label={label}
        {...(pressed !== undefined && { "aria-pressed": pressed })}
        className={`icon-btn icon-btn--${tone}`}
      >
        <Icon name={icon} />
      </AriaButton>
      <Tooltip className="tooltip" offset={6}>
        {label}
      </Tooltip>
    </TooltipTrigger>
  );
}
