# AfterBabel — Landing page

Pre-launch waitlist page for AfterBabel.

Stack: Next.js 15 (App Router) · React 19 · Tailwind v4 · TypeScript.

## Local dev

```bash
cd web
npm install
npm run dev
```

Open <http://localhost:3000>.

## Deploy

Vercel project pointed at the `web/` directory. No env vars are required for
the page to render. Two are read on the server:

| Var | What it does |
|---|---|
| `WAITLIST_ENDPOINT` | URL the API route forwards waitlist submissions to (Resend, ConvertKit, Supabase, internal). When unset, submissions are accepted but not persisted. |
| `WAITLIST_API_KEY`  | Bearer token attached to forwarded submissions. |

## Where things live

| File | Section |
|---|---|
| `app/page.tsx` | Page composition (order of sections) |
| `app/layout.tsx` | Metadata, OG tags, fonts |
| `app/globals.css` | Tailwind v4 theme tokens, animations |
| `app/api/waitlist/route.ts` | Edge route that validates + forwards submissions |
| `app/thanks/page.tsx` | Standalone confirmation page |
| `app/privacy/page.tsx` | Pre-launch privacy summary |
| `components/Hero.tsx` | Above-the-fold |
| `components/DemoPlayer.tsx` | Audio + synced bilingual captions (drives ~50% of conversion per the spec) |
| `components/ProblemTable.tsx` | Comparison: subtitles / robot voiceover / interpreter / AfterBabel |
| `components/HowItWorks.tsx` | Three-step explainer |
| `components/UseCases.tsx` | Four scenarios |
| `components/TechCredibility.tsx` | Stats + stack + privacy stance |
| `components/FounderNote.tsx` | Personal note (placeholder until real founder copy lands) |
| `components/WaitlistForm.tsx` | Email + use-case picker + role |

## Before launch

See [`CONTENT.md`](./CONTENT.md) for the placeholder checklist (demo audio,
founder copy + photo, social handles, domain).
