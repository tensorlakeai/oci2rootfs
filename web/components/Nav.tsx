import Link from "next/link";
import { Container } from "./Container";
import { Wordmark } from "./Wordmark";

export function Nav() {
  return (
    <header className="sticky top-0 z-30 border-b border-[var(--color-border)]/40 bg-[var(--color-bg)]/70 backdrop-blur-xl">
      <Container className="flex h-14 items-center justify-between">
        <Link href="/" className="flex items-center gap-2 text-[15px]">
          <Wordmark />
          <span className="font-display tracking-tight">AfterBabel</span>
        </Link>
        <nav className="flex items-center gap-1 text-sm">
          <a
            href="#demo"
            className="hidden rounded-md px-3 py-1.5 text-[var(--color-fg-muted)] transition-colors hover:text-[var(--color-fg)] sm:inline"
          >
            Demo
          </a>
          <a
            href="#how"
            className="hidden rounded-md px-3 py-1.5 text-[var(--color-fg-muted)] transition-colors hover:text-[var(--color-fg)] sm:inline"
          >
            How it works
          </a>
          <a
            href="#waitlist"
            className="ml-1 rounded-md border border-[var(--color-border-strong)] bg-[var(--color-surface)] px-3 py-1.5 text-[var(--color-fg)] transition-colors hover:bg-[var(--color-bg-elevated)]"
          >
            Join waitlist
          </a>
        </nav>
      </Container>
    </header>
  );
}
