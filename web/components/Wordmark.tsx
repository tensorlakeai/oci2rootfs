export function Wordmark({ size = 22 }: { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      role="img"
      aria-label="AfterBabel mark"
      className="text-[var(--color-accent)]"
    >
      <g fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round">
        {/* Two voices, one waveform */}
        <path d="M3 12h2" />
        <path d="M6.5 9v6" />
        <path d="M9.5 6.5v11" />
        <path d="M12.5 4v16" opacity="0.85" />
        <path d="M15.5 6.5v11" />
        <path d="M18.5 9v6" />
        <path d="M21 12h-2" />
      </g>
    </svg>
  );
}
