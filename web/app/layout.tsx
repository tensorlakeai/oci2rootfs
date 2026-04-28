import type { Metadata, Viewport } from "next";
import "./globals.css";

const SITE_URL = "https://afterbabel.ai";
const TITLE = "AfterBabel — Voice-preserving real-time interpretation";
const DESCRIPTION =
  "Real-time interpretation that sounds like the person speaking. Their voice, your language. Sub-second latency, no host bot.";

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  title: {
    default: TITLE,
    template: "%s — AfterBabel",
  },
  description: DESCRIPTION,
  applicationName: "AfterBabel",
  keywords: [
    "voice-preserving translation",
    "real-time interpretation",
    "cross-language voice cloning",
    "simultaneous interpretation API",
    "ElevenLabs Scribe",
    "AfterBabel",
  ],
  authors: [{ name: "AfterBabel" }],
  openGraph: {
    title: TITLE,
    description: DESCRIPTION,
    url: SITE_URL,
    siteName: "AfterBabel",
    type: "website",
    images: [{ url: "/og.svg", width: 1200, height: 630, alt: "AfterBabel" }],
  },
  twitter: {
    card: "summary_large_image",
    title: TITLE,
    description: DESCRIPTION,
    images: ["/og.svg"],
  },
  icons: {
    icon: [{ url: "/favicon.svg", type: "image/svg+xml" }],
  },
  robots: { index: true, follow: true },
};

export const viewport: Viewport = {
  themeColor: "#0d1410",
  width: "device-width",
  initialScale: 1,
};

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <head>
        <link
          rel="preconnect"
          href="https://rsms.me/"
          crossOrigin="anonymous"
        />
        <link rel="stylesheet" href="https://rsms.me/inter/inter.css" />
      </head>
      <body className="min-h-screen antialiased">{children}</body>
    </html>
  );
}
