import { Container } from "./Container";

const CASES = [
  {
    tag: "Sales",
    title: "Cross-border sales calls",
    body: "Your English doesn’t have to be perfect. The buyer hears your own voice — your tone, your hesitations, your conviction — in their language.",
  },
  {
    tag: "Hiring",
    title: "International recruiting",
    body: "Stop filtering candidates by their second language. Let them show you who they are in the language they think in.",
  },
  {
    tag: "Support",
    title: "Overseas customer support",
    body: "Your team replies in their native tongue. The customer hears a fluent local voice that still sounds like a person, not a help-desk bot.",
  },
  {
    tag: "Collab",
    title: "International project reviews",
    body: "Design critique, technical deep-dives, contract negotiation — the moments where nuance matters and broken English costs you the room.",
  },
] as const;

export function UseCases() {
  return (
    <section className="border-y border-[var(--color-border)]/60 bg-[var(--color-bg-elevated)]/30 py-20 sm:py-28">
      <Container>
        <div className="mx-auto max-w-2xl text-center">
          <p className="mb-3 text-xs font-mono uppercase tracking-[0.18em] text-[var(--color-fg-subtle)]">
            Where it matters
          </p>
          <h2 className="font-display text-3xl tracking-tight sm:text-4xl">
            Built for the calls where the stakes are real.
          </h2>
        </div>

        <div className="mt-14 grid gap-5 sm:grid-cols-2">
          {CASES.map((c) => (
            <article
              key={c.title}
              className="group relative overflow-hidden rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)]/50 p-6 transition-colors hover:border-[var(--color-border-strong)]"
            >
              <span className="font-mono text-[11px] uppercase tracking-[0.18em] text-[var(--color-accent)]">
                {c.tag}
              </span>
              <h3 className="mt-3 font-display text-2xl tracking-tight">
                {c.title}
              </h3>
              <p className="mt-3 text-[15px] leading-relaxed text-[var(--color-fg-muted)]">
                {c.body}
              </p>
            </article>
          ))}
        </div>
      </Container>
    </section>
  );
}
