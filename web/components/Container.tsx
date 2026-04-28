import { cn } from "@/lib/cn";

export function Container({
  children,
  className,
  as: As = "div",
}: {
  children: React.ReactNode;
  className?: string;
  as?: "div" | "section" | "header" | "footer" | "main";
}) {
  return (
    <As className={cn("mx-auto w-full max-w-6xl px-6 md:px-10", className)}>
      {children}
    </As>
  );
}
