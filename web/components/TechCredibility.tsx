import { Container } from "./Container";

const STATS = [
  { value: "<1s", label: "End-to-end latency", caption: "Mic to translated voice." },
  { value: "90+", label: "Languages recognized", caption: "Powered by Scribe v2 Realtime." },
  { value: "4", label: "Language pairs at MVP", caption: "EN–ZH, JA–ZH, KO–ZH, JA–EN." },
  { value: "30s", label: "Voice sample to clone", caption: "No training wait, no enrollment." },
];

export function TechCredibility() {
  return (
    <section className="py-20 sm:py-28">
      <Container>
        <div className="grid gap-12 lg:grid-cols-12">
          <div className="lg:col-span-5">
            <p className="mb-3 text-xs font-mono uppercase tracking-[0.18em] text-[var(--color-fg-subtle)]">
              Under the hood
            </p>
            <h2 className="font-display text-3xl tracking-tight sm:text-4xl">
              The technology stack <br />
              <span className="text-[var(--color-fg-muted)]">
                that finally makes this possible.
              </span>
            </h2>
            <p className="mt-5 max-w-md text-[15px] leading-relaxed text-[var(--color-fg-muted)]">
              ElevenLabs’ Scribe v2 Realtime gets us under 150ms transcription.
              Flash v2.5 gets us under 500ms TTS. The window where this is a real
              software product — not a research demo — opened in 2025.
            </p>
            <ul className="mt-8 space-y-3 text-[14px] text-[var(--color-fg-muted)]">
              <li className="flex items-start gap-3">
                <Check />
                <span>Audio processed in real time. Never stored on our servers.</span>
              </li>
              <li className="flex items-start gap-3">
                <Check />
                <span>Voice samples stay encrypted, scoped to your account, and are deletable on request.</span>
              </li>
              <li className="flex items-start gap-3">
                <Check />
                <span>SOC 2 Type II — in progress. Targeting H2.</span>
              </li>
            </ul>
          </div>

          <div className="lg:col-span-7">
            <div className="grid grid-cols-2 gap-3">
              {STATS.map((s) => (
                <div
                  key={s.label}
                  className="rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)]/40 p-6"
                >
                  <div className="font-display text-4xl tracking-tight sm:text-5xl">
                    {s.value}
                  </div>
                  <div className="mt-3 text-[13px] font-mono uppercase tracking-[0.14em] text-[var(--color-fg-subtle)]">
                    {s.label}
                  </div>
                  <p className="mt-2 text-[14px] text-[var(--color-fg-muted)]">
                    {s.caption}
                  </p>
                </div>
              ))}
            </div>

            <div className="mt-3 rounded-xl border border-[var(--color-border)] bg-[var(--color-surface)]/40 p-5 text-[13px] font-mono text-[var(--color-fg-subtle)]">
              <span className="text-[var(--color-fg-muted)]">stack</span>{" "}
              · ElevenLabs Scribe v2 Realtime · Flash v2.5 · cross-language
              voice cloning · custom audio routing layer
            </div>
          </div>
        </div>
      </Container>
    </section>
  );
}

function Check() {
  return (
    <svg
      width="16"
      height="16"
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden
      className="mt-0.5 shrink-0 text-[var(--color-accent)]"
    >
      <path
        d="M3 8.5l3 3 7-7"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
