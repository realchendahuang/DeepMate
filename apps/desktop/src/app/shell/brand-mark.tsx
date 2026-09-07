import { cn } from "@/shared/lib/utils";

// Square app icon. Never wrap this in a circle — the asset is already a
// rounded square, and a circular well inverts or clips it.
export function BrandMark({
  className,
  wordmark = false,
  alt = "DeepMate",
}: {
  className?: string;
  wordmark?: boolean;
  alt?: string;
}) {
  return (
    <span className={cn("inline-flex items-center gap-2.5", className)}>
      <img
        src="/logo.png"
        alt={wordmark ? "" : alt}
        className="h-full w-auto rounded-2xl object-cover dark:hidden"
      />
      <img
        src="/logo-dark.png"
        alt=""
        aria-hidden
        className="hidden h-full w-auto rounded-2xl object-cover dark:inline"
      />
      {wordmark && <span className="text-brand text-text">DeepMate</span>}
    </span>
  );
}
