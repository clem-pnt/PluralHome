/** Icônes au trait (24×24), dessinées à la main pour rester hors ligne et légères. */
const PATHS = {
  edit: "M4 20h4L19 9a2.8 2.8 0 0 0-4-4L4 16v4ZM13.5 6.5l4 4",
  trash: "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3",
  check: "M5 12.5l4.5 4.5L19 7.5",
  checks: "M2 12.5l4.5 4.5L16 7.5M12 16.5l.5.5L22 7.5",
  search: "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14ZM20 20l-4-4",
  users:
    "M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8ZM2 21v-1a6 6 0 0 1 6-6h2a6 6 0 0 1 6 6v1M16 3.5a4 4 0 0 1 0 7M18 14a6 6 0 0 1 4 6v1",
  folderPlus:
    "M3 6a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6ZM12 10v6M9 13h6",
  star: "M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8L3.5 9.7l5.9-.9L12 3.5Z",
  history: "M3 12a9 9 0 1 0 3-6.7L3 8M3 3v5h5M12 7v5l3 2",
  close: "M6 6l12 12M18 6 6 18",
  plus: "M12 5v14M5 12h14",
  pin: "M9 4h6l-1 6 4 3H6l4-3-1-6ZM12 13v7",
  lock: "M6 11h12v9H6v-9ZM8 11V8a4 4 0 0 1 8 0v3",
  unlock: "M6 11h12v9H6v-9ZM8 11V8a4 4 0 0 1 7.6-1.8",
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({ name, size = 18 }: { name: IconName; size?: number }) {
  return (
    <svg
      className="icon"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
