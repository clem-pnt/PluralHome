import { Button as AriaButton, type ButtonProps as AriaButtonProps } from "react-aria-components";

type Variant = "primary" | "accent" | "ghost" | "danger";

export interface ButtonProps extends Omit<AriaButtonProps, "className"> {
  variant?: Variant;
  block?: boolean;
}

export function Button({ variant = "primary", block = false, ...props }: ButtonProps) {
  const cls = ["btn", `btn--${variant}`, block && "btn--block"].filter(Boolean).join(" ");
  return <AriaButton {...props} className={cls} />;
}
