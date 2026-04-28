import { NextRequest, NextResponse } from "next/server";

export const runtime = "edge";

type Payload = {
  email?: string;
  useCase?: string;
  role?: string;
};

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const ALLOWED_USE_CASES = new Set([
  "sales",
  "hiring",
  "support",
  "collab",
  "personal",
  "other",
]);

export async function POST(req: NextRequest) {
  let body: Payload;
  try {
    body = (await req.json()) as Payload;
  } catch {
    return NextResponse.json({ error: "Invalid JSON" }, { status: 400 });
  }

  const email = (body.email ?? "").trim().toLowerCase();
  const useCase = (body.useCase ?? "").trim();
  const role = (body.role ?? "").trim().slice(0, 200);

  if (!email || !EMAIL_RE.test(email) || email.length > 200) {
    return NextResponse.json(
      { error: "Please enter a valid email address." },
      { status: 400 },
    );
  }
  if (useCase && !ALLOWED_USE_CASES.has(useCase)) {
    return NextResponse.json({ error: "Invalid use case." }, { status: 400 });
  }

  // Forward to a downstream provider when configured. Until that's wired up
  // we accept the submission so the page is fully testable; nothing is stored
  // server-side. See web/README.md for provider hookup.
  const endpoint = process.env.WAITLIST_ENDPOINT;
  const apiKey = process.env.WAITLIST_API_KEY;

  if (endpoint) {
    try {
      const r = await fetch(endpoint, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          ...(apiKey ? { authorization: `Bearer ${apiKey}` } : {}),
        },
        body: JSON.stringify({ email, useCase, role, source: "landing" }),
      });
      if (!r.ok) {
        return NextResponse.json(
          { error: "Could not record submission. Please try again." },
          { status: 502 },
        );
      }
    } catch {
      return NextResponse.json(
        { error: "Network error. Please try again." },
        { status: 502 },
      );
    }
  }

  return NextResponse.json({ ok: true });
}
