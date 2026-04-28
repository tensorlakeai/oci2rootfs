"use client";

import { useState } from "react";
import { Container } from "./Container";

const USE_CASES = [
  { value: "sales", label: "Cross-border sales calls" },
  { value: "hiring", label: "International recruiting" },
  { value: "support", label: "Overseas customer support" },
  { value: "collab", label: "International project collaboration" },
  { value: "personal", label: "Personal / professional 1:1 calls" },
  { value: "other", label: "Something else" },
] as const;

type Status = "idle" | "submitting" | "ok" | "error";

export function WaitlistForm() {
  const [status, setStatus] = useState<Status>("idle");
  const [error, setError] = useState<string>("");
  const [useCase, setUseCase] = useState<string>("sales");

  const onSubmit = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (status === "submitting") return;
    setStatus("submitting");
    setError("");

    const form = e.currentTarget;
    const data = new FormData(form);
    const payload = {
      email: String(data.get("email") || "").trim(),
      useCase: String(data.get("useCase") || ""),
      role: String(data.get("role") || "").trim(),
    };

    try {
      const res = await fetch("/api/waitlist", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(payload),
      });
      if (!res.ok) {
        const j = await res.json().catch(() => ({}));
        throw new Error(j?.error || "Could not submit. Please try again.");
      }
      setStatus("ok");
      form.reset();
    } catch (err) {
      setStatus("error");
      setError(err instanceof Error ? err.message : "Could not submit.");
    }
  };

  return (
    <section
      id="waitlist"
      className="scroll-mt-20 border-y border-[var(--color-border)]/60 bg-[var(--color-bg-elevated)]/30 py-20 sm:py-28"
    >
      <Container>
        <div className="mx-auto grid max-w-5xl gap-12 lg:grid-cols-12">
          <div className="lg:col-span-5">
            <p className="mb-3 text-xs font-mono uppercase tracking-[0.18em] text-[var(--color-accent)]">
              Private beta
            </p>
            <h2 className="font-display text-3xl tracking-tight sm:text-4xl">
              Get on the list.
            </h2>
            <p className="mt-5 text-[15px] leading-relaxed text-[var(--color-fg-muted)]">
              We’re inviting cohorts of 100. The first hundred users get free
              access while we calibrate the product against real calls. Tell us
              what you’d use it for and we’ll prioritize accordingly.
            </p>
            <ul className="mt-8 space-y-3 text-[14px] text-[var(--color-fg-muted)]">
              <li className="flex items-start gap-3"><Dot />macOS first, Windows in the same cohort.</li>
              <li className="flex items-start gap-3"><Dot />No company-email gate. No sales call.</li>
              <li className="flex items-start gap-3"><Dot />We email once when your cohort opens — that’s it.</li>
            </ul>
          </div>

          <div className="lg:col-span-7">
            {status === "ok" ? (
              <SuccessCard />
            ) : (
              <form
                onSubmit={onSubmit}
                className="rounded-2xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)]/60 p-6 sm:p-8"
              >
                <div className="space-y-5">
                  <Field
                    id="email"
                    label="Email"
                    required
                    type="email"
                    autoComplete="email"
                    placeholder="you@somewhere.com"
                  />

                  <fieldset>
                    <legend className="text-[13px] text-[var(--color-fg-muted)]">
                      What would you use this for?
                    </legend>
                    <input type="hidden" name="useCase" value={useCase} />
                    <div className="mt-3 grid gap-2 sm:grid-cols-2">
                      {USE_CASES.map((u) => {
                        const active = u.value === useCase;
                        return (
                          <button
                            type="button"
                            key={u.value}
                            onClick={() => setUseCase(u.value)}
                            className={
                              "w-full rounded-md border px-4 py-3 text-left text-[14px] transition-colors " +
                              (active
                                ? "border-[var(--color-accent)]/60 bg-[var(--color-accent-soft)]/30 text-[var(--color-fg)]"
                                : "border-[var(--color-border)] bg-[var(--color-surface)]/40 text-[var(--color-fg-muted)] hover:border-[var(--color-border-strong)] hover:text-[var(--color-fg)]")
                            }
                            aria-pressed={active}
                          >
                            {u.label}
                          </button>
                        );
                      })}
                    </div>
                  </fieldset>

                  <Field
                    id="role"
                    label="Your role / company (optional)"
                    type="text"
                    placeholder="e.g. AE at a B2B SaaS, Tokyo"
                    optional
                  />
                </div>

                <div className="mt-7 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                  <p className="text-[12px] text-[var(--color-fg-subtle)]">
                    We’ll never share your email. Unsubscribe in one click.
                  </p>
                  <button
                    type="submit"
                    disabled={status === "submitting"}
                    className="inline-flex items-center justify-center gap-2 rounded-md bg-[var(--color-accent)] px-5 py-3 text-sm font-medium text-[#0d1410] transition-colors hover:bg-[#8ec8a7] disabled:opacity-60"
                  >
                    {status === "submitting" ? "Joining…" : "Join the waitlist"}
                  </button>
                </div>

                {status === "error" && (
                  <p className="mt-4 rounded-md border border-red-500/30 bg-red-500/5 px-3 py-2 text-[13px] text-red-300">
                    {error}
                  </p>
                )}
              </form>
            )}
          </div>
        </div>
      </Container>
    </section>
  );
}

function Field({
  id,
  label,
  type,
  placeholder,
  required,
  optional,
  autoComplete,
}: {
  id: string;
  label: string;
  type: string;
  placeholder?: string;
  required?: boolean;
  optional?: boolean;
  autoComplete?: string;
}) {
  return (
    <div>
      <label
        htmlFor={id}
        className="flex items-center justify-between text-[13px] text-[var(--color-fg-muted)]"
      >
        <span>{label}</span>
        {optional && (
          <span className="text-[11px] text-[var(--color-fg-subtle)]">optional</span>
        )}
      </label>
      <input
        id={id}
        name={id}
        type={type}
        required={required}
        placeholder={placeholder}
        autoComplete={autoComplete}
        className="mt-2 w-full rounded-md border border-[var(--color-border)] bg-[var(--color-surface)]/60 px-4 py-3 text-[15px] text-[var(--color-fg)] placeholder:text-[var(--color-fg-subtle)] focus:border-[var(--color-accent)]/60 focus:outline-none"
      />
    </div>
  );
}

function SuccessCard() {
  const text = encodeURIComponent(
    "Just joined the AfterBabel waitlist — real-time interpretation that keeps the speaker’s actual voice. Their voice, your language."
  );
  return (
    <div className="rounded-2xl border border-[var(--color-accent)]/30 bg-[var(--color-accent-soft)]/20 p-8 text-center">
      <div className="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-[var(--color-accent)]/20 text-[var(--color-accent)]">
        <svg width="22" height="22" viewBox="0 0 22 22" fill="none" aria-hidden>
          <path
            d="M5 11.5l3.5 3.5 8-9"
            stroke="currentColor"
            strokeWidth="1.8"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </div>
      <h3 className="mt-5 font-display text-2xl tracking-tight">You’re on the list.</h3>
      <p className="mx-auto mt-3 max-w-md text-[15px] text-[var(--color-fg-muted)]">
        We’ll email when your cohort opens. In the meantime — if you’d post
        about it, it genuinely helps us prioritize the right languages first.
      </p>
      <div className="mt-6 flex flex-wrap items-center justify-center gap-2">
        <a
          href={`https://twitter.com/intent/tweet?text=${text}`}
          target="_blank"
          rel="noopener noreferrer"
          className="rounded-md border border-[var(--color-border-strong)] bg-[var(--color-surface)] px-4 py-2 text-[13px] hover:bg-[var(--color-bg-elevated)]"
        >
          Share on X
        </a>
        <a
          href={`https://www.linkedin.com/sharing/share-offsite/?url=https%3A%2F%2Fafterbabel.ai`}
          target="_blank"
          rel="noopener noreferrer"
          className="rounded-md border border-[var(--color-border-strong)] bg-[var(--color-surface)] px-4 py-2 text-[13px] hover:bg-[var(--color-bg-elevated)]"
        >
          Share on LinkedIn
        </a>
      </div>
    </div>
  );
}

function Dot() {
  return (
    <span
      aria-hidden
      className="mt-2 block h-1 w-1 shrink-0 rounded-full bg-[var(--color-accent)]"
    />
  );
}
