import Link from "next/link";
import { Container } from "./Container";
import { Wordmark } from "./Wordmark";

export function Footer() {
  return (
    <footer className="border-t border-[var(--color-border)]/60 py-12 text-[13px] text-[var(--color-fg-subtle)]">
      <Container className="flex flex-col items-start gap-6 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-center gap-2">
          <Wordmark size={18} />
          <span className="font-display tracking-tight text-[var(--color-fg-muted)]">
            AfterBabel
          </span>
          <span aria-hidden className="mx-2 text-[var(--color-fg-subtle)]/40">
            ·
          </span>
          <span>© {new Date().getFullYear()}</span>
        </div>
        <nav className="flex flex-wrap items-center gap-x-6 gap-y-2">
          <a
            href="https://x.com/afterbabel"
            target="_blank"
            rel="noopener noreferrer"
            className="hover:text-[var(--color-fg-muted)]"
          >
            X
          </a>
          <a
            href="mailto:hello@afterbabel.ai"
            className="hover:text-[var(--color-fg-muted)]"
          >
            hello@afterbabel.ai
          </a>
          <Link
            href="/privacy"
            className="hover:text-[var(--color-fg-muted)]"
          >
            Privacy
          </Link>
        </nav>
      </Container>
    </footer>
  );
}
