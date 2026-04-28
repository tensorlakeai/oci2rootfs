import Link from "next/link";
import { Container } from "@/components/Container";
import { Footer } from "@/components/Footer";
import { Nav } from "@/components/Nav";

export const metadata = {
  title: "Privacy",
  description: "How AfterBabel handles your audio and account data.",
};

export default function PrivacyPage() {
  return (
    <>
      <Nav />
      <main className="py-16 sm:py-24">
        <Container className="max-w-2xl">
          <p className="text-xs font-mono uppercase tracking-[0.18em] text-[var(--color-fg-subtle)]">
            Privacy
          </p>
          <h1 className="mt-3 font-display text-4xl tracking-tight">
            What we do with your audio.
          </h1>
          <p className="mt-3 text-[13px] text-[var(--color-fg-subtle)]">
            Pre-launch summary. We will publish a full, lawyer-reviewed policy
            before the product is generally available.
          </p>

          <div className="mt-10 space-y-8 text-[15px] leading-relaxed text-[var(--color-fg-muted)]">
            <Section title="Real-time audio">
              While AfterBabel is running, we stream your call audio through
              transcription and synthesis. We do not retain that audio after the
              call ends. Nothing is written to our long-term storage.
            </Section>
            <Section title="Voice samples">
              The 30-second sample we use for cloning is encrypted at rest,
              scoped to your account, and deletable on request. We do not use
              your samples to train any general-purpose model.
            </Section>
            <Section title="Transcripts">
              When you turn on session transcripts, we store them for you to
              re-read or download later. You can delete them at any time, and
              they are not used for model training.
            </Section>
            <Section title="Compliance">
              SOC 2 Type II is in progress. We can answer security
              questionnaires for early customers — write to{" "}
              <a
                href="mailto:hello@afterbabel.ai"
                className="text-[var(--color-fg)] underline decoration-[var(--color-border-strong)] underline-offset-2 hover:decoration-[var(--color-accent)]"
              >
                hello@afterbabel.ai
              </a>
              .
            </Section>
            <Section title="Contact">
              Privacy questions, deletion requests, or anything weird:{" "}
              <a
                href="mailto:privacy@afterbabel.ai"
                className="text-[var(--color-fg)] underline decoration-[var(--color-border-strong)] underline-offset-2 hover:decoration-[var(--color-accent)]"
              >
                privacy@afterbabel.ai
              </a>
              .
            </Section>
          </div>

          <Link
            href="/"
            className="mt-12 inline-block text-[14px] text-[var(--color-fg-subtle)] hover:text-[var(--color-fg-muted)]"
          >
            ← Back home
          </Link>
        </Container>
      </main>
      <Footer />
    </>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section>
      <h2 className="font-display text-xl tracking-tight text-[var(--color-fg)]">
        {title}
      </h2>
      <div className="mt-2">{children}</div>
    </section>
  );
}
