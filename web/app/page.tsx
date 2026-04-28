import { Nav } from "@/components/Nav";
import { Hero } from "@/components/Hero";
import { DemoPlayer } from "@/components/DemoPlayer";
import { ProblemTable } from "@/components/ProblemTable";
import { HowItWorks } from "@/components/HowItWorks";
import { UseCases } from "@/components/UseCases";
import { TechCredibility } from "@/components/TechCredibility";
import { FounderNote } from "@/components/FounderNote";
import { WaitlistForm } from "@/components/WaitlistForm";
import { Footer } from "@/components/Footer";

export default function HomePage() {
  return (
    <>
      <Nav />
      <main>
        <Hero />
        <DemoPlayer />
        <ProblemTable />
        <HowItWorks />
        <UseCases />
        <TechCredibility />
        <FounderNote />
        <WaitlistForm />
      </main>
      <Footer />
    </>
  );
}
