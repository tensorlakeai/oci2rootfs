"use client";

import { useEffect, useMemo, useRef, useState } from "react";
import { Container } from "./Container";

/**
 * Caption timing data for the demo. Replace this when the real audio is
 * recorded — `audioSrc` and these segments are the only things that need to
 * change. Each segment lives in one of two tracks (zh / en) so the same
 * speaker is shown saying the same content in both languages.
 */
type Segment = {
  track: "zh" | "en";
  start: number; // seconds
  end: number;
  text: string;
};

const SEGMENTS: Segment[] = [
  { track: "zh", start: 0.4,  end: 3.2,  text: "我们的客户大多在亚洲，但我们的设计团队在柏林。" },
  { track: "zh", start: 3.4,  end: 6.6,  text: "每周一次评审，过去总要靠字幕——这次我们试了 AfterBabel。" },
  { track: "zh", start: 6.8,  end: 10.2, text: "她说话的节奏，停顿，甚至那个有点压低的尾音，我都听得清清楚楚。" },
  { track: "zh", start: 10.4, end: 13.6, text: "最让我惊讶的不是翻译准确——是我没再低头看屏幕。" },
  { track: "zh", start: 13.8, end: 15.6, text: "我一直在看着她。" },

  { track: "en", start: 16.0, end: 19.0, text: "Most of our customers are in Asia, but our design team is in Berlin." },
  { track: "en", start: 19.2, end: 22.4, text: "Weekly reviews used to mean staring at captions — this time we tried AfterBabel." },
  { track: "en", start: 22.6, end: 26.0, text: "Her rhythm, the pauses, even that slightly lowered final syllable — I heard all of it." },
  { track: "en", start: 26.2, end: 29.0, text: "What surprised me wasn’t the accuracy. It was that I stopped looking down." },
  { track: "en", start: 29.2, end: 31.0, text: "I just kept watching her." },
];

const DURATION = 31.0; // seconds — total length of the demo file

export function DemoPlayer({ audioSrc = "/demo/afterbabel-demo.mp3" }: { audioSrc?: string }) {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const [t, setT] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [available, setAvailable] = useState<boolean | null>(null);

  // Probe for the audio file existence so we can show a graceful placeholder
  // when the demo asset hasn't been recorded yet.
  useEffect(() => {
    let cancelled = false;
    fetch(audioSrc, { method: "HEAD" })
      .then((r) => !cancelled && setAvailable(r.ok))
      .catch(() => !cancelled && setAvailable(false));
    return () => {
      cancelled = true;
    };
  }, [audioSrc]);

  // Drive the timeline from the audio when available. When not, fall back to a
  // simulated playhead so the captions still demo the experience.
  useEffect(() => {
    if (available) {
      const a = audioRef.current;
      if (!a) return;
      const onTime = () => setT(a.currentTime);
      const onPlay = () => setPlaying(true);
      const onPause = () => setPlaying(false);
      const onEnded = () => {
        setPlaying(false);
        setT(0);
      };
      a.addEventListener("timeupdate", onTime);
      a.addEventListener("play", onPlay);
      a.addEventListener("pause", onPause);
      a.addEventListener("ended", onEnded);
      return () => {
        a.removeEventListener("timeupdate", onTime);
        a.removeEventListener("play", onPlay);
        a.removeEventListener("pause", onPause);
        a.removeEventListener("ended", onEnded);
      };
    }

    if (!playing) return;
    const start = performance.now() - t * 1000;
    let raf = 0;
    const tick = (now: number) => {
      const cur = (now - start) / 1000;
      if (cur >= DURATION) {
        setPlaying(false);
        setT(0);
        return;
      }
      setT(cur);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [available, playing, t]);

  const toggle = () => {
    if (available) {
      const a = audioRef.current;
      if (!a) return;
      if (a.paused) a.play();
      else a.pause();
    } else {
      setPlaying((p) => !p);
    }
  };

  const seek = (next: number) => {
    const clamped = Math.max(0, Math.min(DURATION, next));
    setT(clamped);
    if (available && audioRef.current) audioRef.current.currentTime = clamped;
  };

  const zh = useMemo(() => SEGMENTS.filter((s) => s.track === "zh"), []);
  const en = useMemo(() => SEGMENTS.filter((s) => s.track === "en"), []);
  const progress = (t / DURATION) * 100;

  return (
    <section id="demo" className="relative scroll-mt-20 py-20 sm:py-28">
      <Container>
        <div className="mx-auto max-w-3xl text-center">
          <p className="mb-3 text-xs font-mono uppercase tracking-[0.18em] text-[var(--color-accent)]">
            Demo · 31 seconds
          </p>
          <h2 className="font-display text-3xl tracking-tight sm:text-5xl">
            This is the same person —<br className="hidden sm:block" />{" "}
            <span className="text-[var(--color-fg-muted)]">
              first speaking Mandarin, then English.
            </span>
          </h2>
          <p className="mt-5 text-[15px] text-[var(--color-fg-muted)] sm:text-base">
            No re-record, no voice actor. We sampled her voice for 30 seconds, then
            let her keep talking. The English you hear below is still her voice.
          </p>
        </div>

        <div className="mx-auto mt-12 max-w-4xl">
          <div className="rounded-2xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)]/60 p-6 sm:p-8 shadow-[0_30px_120px_-40px_rgba(0,0,0,0.7)]">
            {available === false ? (
              <PlaceholderBanner />
            ) : null}

            <audio
              ref={audioRef}
              src={audioSrc}
              preload="metadata"
              className="hidden"
            />

            <div className="flex items-center gap-4">
              <button
                onClick={toggle}
                aria-label={playing ? "Pause demo" : "Play demo"}
                className="flex h-12 w-12 shrink-0 items-center justify-center rounded-full bg-[var(--color-accent)] text-[#0d1410] transition-transform hover:scale-[1.04] active:scale-[0.97]"
              >
                {playing ? <PauseGlyph /> : <PlayGlyph />}
              </button>

              <div className="flex-1">
                <Timeline t={t} duration={DURATION} onSeek={seek} progress={progress} />
                <div className="mt-2 flex justify-between font-mono text-[11px] text-[var(--color-fg-subtle)]">
                  <span>{formatTime(t)}</span>
                  <span>{formatTime(DURATION)}</span>
                </div>
              </div>
            </div>

            <div className="mt-8 grid gap-6 sm:grid-cols-2">
              <CaptionTrack
                label="Mandarin · original"
                lang="zh"
                segments={zh}
                t={t}
              />
              <CaptionTrack
                label="English · same voice"
                lang="en"
                segments={en}
                t={t}
                accent
              />
            </div>

            <div className="mt-8 flex flex-wrap items-center justify-between gap-3 border-t border-[var(--color-border)]/60 pt-5 text-xs text-[var(--color-fg-subtle)]">
              <span className="font-mono">
                avg latency <span className="text-[var(--color-fg-muted)]">412ms</span> · Scribe v2 · Flash v2.5
              </span>
              <a
                href={audioSrc}
                download
                className="rounded-md border border-[var(--color-border)] px-3 py-1.5 transition-colors hover:bg-[var(--color-surface)] hover:text-[var(--color-fg-muted)]"
              >
                Download mp3
              </a>
            </div>
          </div>
        </div>
      </Container>
    </section>
  );
}

function CaptionTrack({
  label,
  lang,
  segments,
  t,
  accent,
}: {
  label: string;
  lang: "zh" | "en";
  segments: Segment[];
  t: number;
  accent?: boolean;
}) {
  const activeIdx = segments.findIndex((s) => t >= s.start && t < s.end);
  return (
    <div
      className={
        "rounded-xl border p-5 transition-colors " +
        (accent
          ? "border-[var(--color-accent)]/30 bg-[var(--color-accent-soft)]/20"
          : "border-[var(--color-border)] bg-[var(--color-surface)]/40")
      }
    >
      <div className="mb-3 flex items-center justify-between text-[11px] uppercase tracking-[0.16em]">
        <span
          className={
            "font-mono " +
            (accent ? "text-[var(--color-accent)]" : "text-[var(--color-fg-subtle)]")
          }
        >
          {label}
        </span>
        <span className="font-mono text-[var(--color-fg-subtle)]">
          {lang === "zh" ? "zh-CN" : "en-US"}
        </span>
      </div>
      <ul className="space-y-2.5 text-[15px] leading-relaxed">
        {segments.map((s, i) => {
          const isActive = i === activeIdx;
          const isPast = t >= s.end;
          return (
            <li
              key={i}
              className="caption-line"
              style={{
                color: isActive
                  ? "var(--color-fg)"
                  : isPast
                    ? "var(--color-fg-subtle)"
                    : "var(--color-fg-muted)",
                opacity: isActive ? 1 : 0.85,
              }}
            >
              {isActive && (
                <span
                  aria-hidden
                  className="mr-2 inline-block h-1.5 w-1.5 translate-y-[-2px] rounded-full bg-[var(--color-accent)]"
                />
              )}
              {s.text}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

function Timeline({
  t,
  duration,
  onSeek,
  progress,
}: {
  t: number;
  duration: number;
  onSeek: (n: number) => void;
  progress: number;
}) {
  const ref = useRef<HTMLDivElement | null>(null);
  const handle = (clientX: number) => {
    const el = ref.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const ratio = (clientX - rect.left) / rect.width;
    onSeek(ratio * duration);
  };
  return (
    <div
      ref={ref}
      role="slider"
      aria-label="Demo timeline"
      aria-valuemin={0}
      aria-valuemax={duration}
      aria-valuenow={t}
      tabIndex={0}
      onKeyDown={(e) => {
        if (e.key === "ArrowRight") onSeek(t + 1);
        if (e.key === "ArrowLeft") onSeek(t - 1);
      }}
      onClick={(e) => handle(e.clientX)}
      className="group relative h-2 cursor-pointer rounded-full bg-[var(--color-border)]"
    >
      <div
        className="absolute inset-y-0 left-0 rounded-full bg-[var(--color-accent)] transition-[width] duration-100"
        style={{ width: `${progress}%` }}
      />
      <div
        className="absolute top-1/2 h-3 w-3 -translate-x-1/2 -translate-y-1/2 rounded-full bg-[var(--color-fg)] opacity-0 transition-opacity group-hover:opacity-100"
        style={{ left: `${progress}%` }}
      />
    </div>
  );
}

function PlaceholderBanner() {
  return (
    <div className="mb-5 rounded-md border border-[var(--color-warn)]/40 bg-[var(--color-warn)]/5 px-4 py-3 text-[13px] text-[var(--color-warn)]">
      Demo audio not yet attached. Captions below run on a simulated timeline so
      the experience is testable. Drop the file at{" "}
      <code className="font-mono text-[12px]">public/demo/afterbabel-demo.mp3</code>{" "}
      and reload.
    </div>
  );
}

function formatTime(s: number) {
  const m = Math.floor(s / 60);
  const r = Math.floor(s % 60);
  return `${m}:${r.toString().padStart(2, "0")}`;
}

function PlayGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor" aria-hidden>
      <path d="M3 1.8a.6.6 0 0 1 .92-.5l8 5.2a.6.6 0 0 1 0 1l-8 5.2A.6.6 0 0 1 3 12.2z" />
    </svg>
  );
}

function PauseGlyph() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" fill="currentColor" aria-hidden>
      <rect x="3" y="2" width="3" height="10" rx="1" />
      <rect x="8" y="2" width="3" height="10" rx="1" />
    </svg>
  );
}
