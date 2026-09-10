export function Shortcut({ compact = false }: { compact?: boolean }) {
  return (
    <span
      className={`shortcut${compact ? " shortcut-compact" : ""}`}
      aria-label="Control plus Alt plus F8"
    >
      <kbd>Ctrl</kbd>
      <span>+</span>
      <kbd>Alt</kbd>
      <span>+</span>
      <kbd>F8</kbd>
    </span>
  );
}
