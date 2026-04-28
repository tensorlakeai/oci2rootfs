# AfterBabel landing — content checklist

Items below need real content before launch. Until they're filled in, the page
is fully functional but uses tasteful placeholders.

## Critical (drives conversion)

- **Demo audio** — drop file at `public/demo/afterbabel-demo.mp3`. 31-second
  clip, same voice in Mandarin then English. The page auto-detects the file;
  if missing, captions still play on a simulated timeline with a small banner.
- **Demo captions** — edit the `SEGMENTS` array at the top of
  `components/DemoPlayer.tsx`. Each entry is `{ track, start, end, text }`.
  Adjust `DURATION` to match the audio length.

## Visible placeholders

- **Founder photo** — `components/FounderNote.tsx` renders an SVG silhouette.
  Replace with a real photograph (not AI-generated, not stock). Recommend a
  64×64 jpg/png in `public/founder.jpg` and swap the `<FounderAvatar>` body.
- **Founder note copy** — currently a generic version. Replace the three
  paragraphs with the founder's own story.
- **Founder name + role line** — the `<div>` reading "The founder" /
  "AfterBabel · founder note".

## Backend wiring

- **Waitlist storage** — `app/api/waitlist/route.ts` accepts submissions but
  only forwards them when `WAITLIST_ENDPOINT` is set. Point it at Resend,
  ConvertKit, Supabase, or an internal endpoint. Use `WAITLIST_API_KEY` for
  bearer auth. Until then, submissions are accepted but not persisted.

## Brand decisions still open

- **Final name** — currently `AfterBabel` everywhere. If `Shinar` wins, do a
  global find/replace in `app/`, `components/`, `public/og.svg`,
  `public/favicon.svg`, and the `metadata` block in `app/layout.tsx`.
- **Domain** — `afterbabel.ai` is hard-coded as the canonical URL in
  `app/layout.tsx` (`SITE_URL`) and OG share targets in
  `components/WaitlistForm.tsx` and `components/Footer.tsx`.
- **Social handles** — `Footer.tsx` links `x.com/afterbabel` and the LinkedIn
  share URL points at the canonical domain. Update both once the accounts
  exist.

## Nice-to-haves

- **OG image as PNG** — `public/og.svg` works for most platforms but X's older
  card validator prefers PNG. Render to 1200×630 PNG before Product Hunt /
  HN day if engagement matters.
- **Privacy policy** — `app/privacy/page.tsx` is a pre-launch summary, not a
  legal document. Have counsel write the GA version.
