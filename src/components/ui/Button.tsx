import type { ButtonHTMLAttributes, ReactNode } from "react";

type Variant = "primary" | "secondary" | "ghost" | "danger";

const styles: Record<Variant, string> = {
  primary:
    "bg-accent text-on-accent shadow-[0_6px_14px_color-mix(in_srgb,var(--accent)_22%,transparent)] hover:bg-accent-deep hover:-translate-y-px",
  secondary: "border border-line bg-paper text-ink-soft hover:border-accent hover:text-accent-deep",
  ghost: "text-accent hover:text-accent-deep px-0!",
  danger: "bg-[#b0413e] text-white hover:bg-[#93322f]",
};

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  icon?: ReactNode;
  size?: "sm" | "md";
}

export function Button({ variant = "primary", icon, size = "md", className = "", children, ...rest }: Props) {
  const pad = size === "sm" ? "px-3 py-1.5 text-[11.5px]" : "px-4 py-2.5 text-[12.5px]";
  return (
    <button
      type="button"
      className={`inline-flex items-center justify-center gap-2 rounded-[11px] font-semibold transition-all duration-150 disabled:hover:translate-y-0 ${pad} ${styles[variant]} ${className}`}
      {...rest}
    >
      {icon}
      {children}
    </button>
  );
}
