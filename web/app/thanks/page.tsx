import Link from "next/link";
import { Container } from "@/components/Container";
import { Wordmark } from "@/components/Wordmark";

export const metadata = {
  title: "You're on the list",
  description: "Thanks for joining the AfterBabel waitlist.",
  robots: { index: false, follow: false },
};

export default function ThanksPage() {
  return (
    <main className="min-h-screen flex items-center">
      <Container className="py-24 text-center">
        <div className="mx-auto flex max-w-md flex-col items-center">
          <Wordmark size={28} />
          <h1 className="mt-6 font-display text-4xl tracking-tight">
            You’re on the list.
          </h1>
          <p className="mt-4 text-[15px] text-[var(--color-fg-muted)]">
            We’ll email when your cohort opens. We promise not to spam — one
            invitation, then we leave you alone.
          </p>
          <Link
            href="/"
            className="mt-8 rounded-md border border-[var(--color-border-strong)] bg-[var(--color-surface)] px-4 py-2 text-sm text-[var(--color-fg)] hover:bg-[var(--color-bg-elevated)]"
          >
            ← Back home
          </Link>
        </div>
      </Container>
    </main>
  );
}
