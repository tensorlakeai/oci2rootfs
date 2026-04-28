import { Container } from "./Container";

export function FounderNote() {
  return (
    <section className="border-t border-[var(--color-border)]/60 py-20 sm:py-28">
      <Container>
        <div className="mx-auto max-w-2xl">
          <p className="mb-6 text-xs font-mono uppercase tracking-[0.18em] text-[var(--color-fg-subtle)]">
            From the team
          </p>

          <div className="flex items-center gap-4">
            <FounderAvatar />
            <div>
              <div className="font-display text-lg tracking-tight">
                {/* Replace before launch */}
                The founder
              </div>
              <div className="text-[13px] text-[var(--color-fg-subtle)]">
                AfterBabel · founder note
              </div>
            </div>
          </div>

          <div className="mt-8 space-y-5 text-[16px] leading-relaxed text-[var(--color-fg-muted)]">
            <p>
              I’ve spent most of my working life on calls where one side is
              speaking their second or third language. I’ve watched brilliant
              people sound hesitant. I’ve been the brilliant person who sounded
              hesitant. The translator in the room — human or machine — was
              never the version of us we wanted to be.
            </p>
            <p>
              For years, “real-time, voice-preserving interpretation” was a
              research-paper sentence. Then Scribe v2 went under 150ms, Flash
              v2.5 went under 500ms, and cross-language cloning got good enough
              that you can’t tell. We finally have a budget to build inside.
            </p>
            <p>
              We’re building AfterBabel slowly and on purpose. The first hundred
              users get our phone numbers. If you’ve ever felt smaller than you
              are because of the language the room was in — please get on the
              list. We’d like to meet you.
            </p>
          </div>

          <p className="mt-8 text-[13px] text-[var(--color-fg-subtle)]">
            {/* Replace before launch — see CONTENT.md */}
            Signed copy and a real photo go here before launch.
          </p>
        </div>
      </Container>
    </section>
  );
}

function FounderAvatar() {
  // Placeholder — replace with a real photograph (not AI-generated, not a
  // stock image) before launch. See web/CONTENT.md.
  return (
    <div className="relative h-14 w-14 shrink-0 overflow-hidden rounded-full border border-[var(--color-border-strong)] bg-[var(--color-surface)]">
      <svg
        viewBox="0 0 56 56"
        className="absolute inset-0 h-full w-full text-[var(--color-fg-subtle)]"
        aria-hidden
      >
        <circle cx="28" cy="22" r="9" fill="currentColor" opacity="0.35" />
        <path
          d="M10 50c2-9 9-14 18-14s16 5 18 14"
          fill="currentColor"
          opacity="0.35"
        />
      </svg>
    </div>
  );
}
