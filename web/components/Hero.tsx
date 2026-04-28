import { Container } from "./Container";

export function Hero() {
  return (
    <section className="relative overflow-hidden pt-12 pb-20 sm:pt-20 sm:pb-28">
      <Container>
        <div className="grid items-center gap-12 lg:grid-cols-12">
          <div className="lg:col-span-7">
            <p className="mb-6 inline-flex items-center gap-2 rounded-full border border-[var(--color-border)] bg-[var(--color-bg-elevated)]/70 px-3 py-1 text-xs text-[var(--color-fg-muted)]">
              <span className="h-1.5 w-1.5 rounded-full bg-[var(--color-accent)]" />
              Private beta — joining cohorts of 100
            </p>
            <h1 className="font-display text-[44px] leading-[1.05] tracking-tight sm:text-[60px] lg:text-[72px]">
              Hear them,
              <br />
              <span className="text-[var(--color-fg-muted)]">not a translator.</span>
            </h1>
            <p className="mt-6 max-w-xl text-[17px] leading-relaxed text-[var(--color-fg-muted)] sm:text-lg">
              Real-time interpretation that sounds like the actual person speaking.
              Their voice, your language. Sub-second latency, no host bot in your
              meeting.
            </p>
            <div className="mt-9 flex flex-col gap-3 sm:flex-row sm:items-center">
              <a
                href="#demo"
                className="inline-flex items-center justify-center gap-2 rounded-md bg-[var(--color-accent)] px-5 py-3 text-sm font-medium text-[#0d1410] transition-transform hover:scale-[1.01] hover:bg-[#8ec8a7] active:scale-[0.99]"
              >
                <PlayIcon />
                Listen to a 30-second demo
              </a>
              <a
                href="#waitlist"
                className="inline-flex items-center justify-center gap-2 rounded-md border border-[var(--color-border-strong)] bg-[var(--color-surface)] px-5 py-3 text-sm text-[var(--color-fg)] transition-colors hover:bg-[var(--color-bg-elevated)]"
              >
                Join waitlist
                <ArrowIcon />
              </a>
            </div>
            <p className="mt-5 text-xs text-[var(--color-fg-subtle)]">
              No credit card. Free for the first 100 users while we calibrate the
              early cohorts.
            </p>
          </div>

          <div className="relative lg:col-span-5">
            <HeroVisual />
          </div>
        </div>
      </Container>
    </section>
  );
}

function HeroVisual() {
  // 32 bars; heights chosen to suggest a real spoken phrase, not a sine wave
  const bars = [
    18, 26, 38, 22, 30, 48, 64, 56, 40, 28, 18, 24, 36, 52, 70, 84, 72, 58, 44,
    34, 26, 20, 28, 42, 58, 70, 80, 68, 50, 36, 24, 16,
  ];

  return (
    <div className="relative aspect-square w-full max-w-md overflow-hidden rounded-2xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)]/60 p-6 shadow-[0_30px_120px_-40px_rgba(0,0,0,0.6)]">
      <div className="flex items-center justify-between text-[11px] tracking-wide text-[var(--color-fg-subtle)] uppercase">
        <span className="font-mono">live</span>
        <span className="font-mono">EN → ZH · 412ms</span>
      </div>

      <div className="mt-10 flex items-end justify-center gap-[3px]" aria-hidden>
        {bars.map((h, i) => (
          <span
            key={i}
            className="wave-bar block w-[6px] rounded-full bg-gradient-to-t from-[var(--color-accent)]/40 to-[var(--color-accent)]"
            style={{
              height: `${h}px`,
              animationDelay: `${i * 35}ms`,
              animationDuration: `${900 + (i % 5) * 80}ms`,
            }}
          />
        ))}
      </div>

      <div className="mt-10 space-y-3 font-mono text-[13px] leading-relaxed">
        <div className="text-[var(--color-fg-subtle)]">
          <span className="mr-2 text-[10px] uppercase tracking-widest text-[var(--color-fg-subtle)]/70">
            heard
          </span>
          “We can ship the pilot in two weeks.”
        </div>
        <div className="text-[var(--color-fg)]">
          <span className="mr-2 text-[10px] uppercase tracking-widest text-[var(--color-accent)]/80">
            spoken — same voice
          </span>
          「我们可以在两周内交付试点。」
        </div>
      </div>

      <div className="pointer-events-none absolute -right-12 -top-12 h-40 w-40 rounded-full bg-[var(--color-accent)]/10 blur-3xl" />
    </div>
  );
}

function PlayIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor" aria-hidden>
      <path d="M3 1.8a.6.6 0 0 1 .92-.5l8 5.2a.6.6 0 0 1 0 1l-8 5.2A.6.6 0 0 1 3 12.2z" />
    </svg>
  );
}

function ArrowIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden>
      <path
        d="M3 7h8M7 3l4 4-4 4"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
