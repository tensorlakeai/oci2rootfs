import { Container } from "./Container";

const STEPS = [
  {
    n: "01",
    title: "Launch AfterBabel and join your call",
    body: "Open the desktop app and pick the call you’re in — Zoom, Google Meet, Teams. Nothing gets installed in their meeting.",
  },
  {
    n: "02",
    title: "AfterBabel takes over the audio",
    body: "It listens, recognizes, translates, and re-speaks each turn — in the original speaker’s voice — straight back into your output.",
  },
  {
    n: "03",
    title: "You both keep talking, in your own languages",
    body: "You hear them in your language. They hear you in theirs. Both still sound like themselves.",
  },
];

export function HowItWorks() {
  return (
    <section id="how" className="scroll-mt-20 py-20 sm:py-28">
      <Container>
        <div className="mx-auto max-w-2xl text-center">
          <p className="mb-3 text-xs font-mono uppercase tracking-[0.18em] text-[var(--color-fg-subtle)]">
            How it works
          </p>
          <h2 className="font-display text-3xl tracking-tight sm:text-4xl">
            One desktop app.
            <br />
            <span className="text-[var(--color-fg-muted)]">
              No host bot, no plugin in their meeting.
            </span>
          </h2>
        </div>

        <ol className="mt-14 grid gap-5 lg:grid-cols-3">
          {STEPS.map((s, i) => (
            <li
              key={s.n}
              className="relative rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)]/40 p-6"
            >
              <div className="flex items-center gap-3">
                <span className="font-mono text-xs text-[var(--color-fg-subtle)]">
                  {s.n}
                </span>
                <span className="h-px flex-1 bg-[var(--color-border)]" />
              </div>
              <h3 className="mt-4 font-display text-xl tracking-tight">
                {s.title}
              </h3>
              <p className="mt-3 text-[15px] leading-relaxed text-[var(--color-fg-muted)]">
                {s.body}
              </p>
              {i === 1 && (
                <p className="mt-4 inline-flex items-center gap-2 rounded-full border border-[var(--color-accent)]/30 bg-[var(--color-accent-soft)]/20 px-3 py-1 text-[12px] text-[var(--color-accent)]">
                  <span className="h-1.5 w-1.5 rounded-full bg-[var(--color-accent)]" />
                  30-second voice sample. No training wait.
                </p>
              )}
            </li>
          ))}
        </ol>
      </Container>
    </section>
  );
}
