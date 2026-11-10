import {
  TextField as AriaTextField,
  type TextFieldProps as AriaTextFieldProps,
  FieldError,
  Input,
  Label,
  Text,
} from "react-aria-components";

export interface TextFieldProps extends Omit<AriaTextFieldProps, "className"> {
  label: string;
  hint?: string | undefined;
}

export function TextField({ label, hint, ...props }: TextFieldProps) {
  return (
    <AriaTextField {...props} className="field">
      <Label className="field__label">{label}</Label>
      <Input className="input" />
      {hint && (
        <Text slot="description" className="field__hint">
          {hint}
        </Text>
      )}
      <FieldError className="field__error" />
    </AriaTextField>
  );
}
