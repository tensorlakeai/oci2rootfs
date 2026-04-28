import { Container } from "./Container";

const ROWS = [
  {
    name: "Subtitles",
    examples: "Otter, Wordly, JotMe",
    limit: "Your eyes leave the person you’re talking to.",
  },
  {
    name: "Robot voiceover",
    examples: "KUDO AI, generic TTS",
    limit: "What you hear is a synthetic voice, not the person.",
  },
  {
    name: "Human interpreter",
    examples: "Agencies, freelancers",
    limit: "Too expensive — and impossible to coordinate — for a 1:1 call.",
  },
  {
    name: "AfterBabel",
    examples: "This page",
    limit: "Their voice, your language.",
    accent: true,
  },
] as const;

export function ProblemTable() {
  return (
    <section className="border-y border-[var(--color-border)]/60 bg-[var(--color-bg-elevated)]/30 py-20 sm:py-28">
      <Container>
        <div className="grid gap-12 lg:grid-cols-12">
          <div className="lg:col-span-5">
            <p className="mb-3 text-xs font-mono uppercase tracking-[0.18em] text-[var(--color-fg-subtle)]">
              The gap nobody is closing
            </p>
            <h2 className="font-display text-3xl tracking-tight sm:text-4xl">
              Most translation tools solve “I can’t understand you.”
              <br />
              <span className="text-[var(--color-fg-muted)]">
                Almost none solve “I can’t connect with you.”
              </span>
            </h2>
            <p className="mt-5 max-w-md text-[15px] text-[var(--color-fg-muted)]">
              In a sales call, an interview, a difficult conversation — the
              difference between a transcript and a voice is the difference
              between processing words and being understood.
            </p>
          </div>

          <div className="lg:col-span-7">
            <div className="overflow-hidden rounded-xl border border-[var(--color-border)]">
              <div className="hidden grid-cols-12 gap-4 border-b border-[var(--color-border)] bg-[var(--color-surface)]/50 px-5 py-3 text-[11px] font-mono uppercase tracking-[0.16em] text-[var(--color-fg-subtle)] sm:grid">
                <div className="col-span-3">Approach</div>
                <div className="col-span-4">Examples</div>
                <div className="col-span-5">What breaks</div>
              </div>
              <ul>
                {ROWS.map((row) => (
                  <li
                    key={row.name}
                    className={
                      "grid gap-2 border-b border-[var(--color-border)] px-5 py-5 last:border-b-0 sm:grid-cols-12 sm:gap-4 sm:py-4 " +
                      ((row as { accent?: boolean }).accent
                        ? "bg-[var(--color-accent-soft)]/25"
                        : "")
                    }
                  >
                    <div className="sm:col-span-3">
                      <span
                        className={
                          "text-sm " +
                          ((row as { accent?: boolean }).accent
                            ? "font-display text-[var(--color-fg)]"
                            : "text-[var(--color-fg)]")
                        }
                      >
                        {row.name}
                      </span>
                    </div>
                    <div className="text-[13px] text-[var(--color-fg-subtle)] sm:col-span-4">
                      {row.examples}
                    </div>
                    <div
                      className={
                        "text-[14px] sm:col-span-5 " +
                        ((row as { accent?: boolean }).accent
                          ? "text-[var(--color-accent)]"
                          : "text-[var(--color-fg-muted)]")
                      }
                    >
                      {row.limit}
                    </div>
                  </li>
                ))}
              </ul>
            </div>
          </div>
        </div>
      </Container>
    </section>
  );
}
