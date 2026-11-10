import {
  Select as AriaSelect,
  Button,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
  SelectValue,
} from "react-aria-components";

export interface Option {
  id: string;
  label: string;
}

/** Liste déroulante accessible. `NONE` représente l'absence de valeur côté métier. */
export function Select({
  label,
  options,
  value,
  onChange,
}: {
  label: string;
  options: Option[];
  value: string;
  onChange: (id: string) => void;
}) {
  return (
    <AriaSelect
      className="field"
      selectedKey={value}
      onSelectionChange={(k) => k !== null && onChange(String(k))}
    >
      <Label className="field__label">{label}</Label>
      <Button className="input select__trigger">
        <SelectValue />
        <span aria-hidden className="select__chevron" />
      </Button>
      <Popover className="popover">
        <ListBox className="listbox" items={options}>
          {(o) => (
            <ListBoxItem id={o.id} className="listbox__item" textValue={o.label}>
              {o.label}
            </ListBoxItem>
          )}
        </ListBox>
      </Popover>
    </AriaSelect>
  );
}
