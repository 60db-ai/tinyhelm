import { useTheme } from "next-themes";
import { cn } from "@/lib/utils";

/** TinyHelm wordmark; swaps to the white-text variant on dark surfaces. */
export function BrandLogo({ className }: { className?: string }) {
  const { resolvedTheme } = useTheme();
  const src = resolvedTheme === "dark" ? "/tinyhelm-logo-dark.png" : "/tinyhelm-logo-light.png";
  return <img src={src} alt="TinyHelm" className={cn("h-8 w-auto shrink-0 self-start object-contain select-none", className)} draggable={false} />;
}
