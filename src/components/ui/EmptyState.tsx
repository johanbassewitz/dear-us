import type { ReactNode } from "react";

interface Props {
  icon?: ReactNode;
  title: string;
  children?: ReactNode;
  action?: ReactNode;
  compact?: boolean;
}

/** Gentle first-time states. Never shows fake data. */
export function EmptyState({ icon, title, children, action, compact }: Props) {
  return (
    <div className={`flex flex-col items-center text-center ${compact ? "gap-2 px-4 py-6" : "gap-3 px-6 py-12"}`}>
      {icon && (
        <div className="grid h-12 w-12 place-items-center rounded-2xl bg-accent-soft text-accent">{icon}</div>
      )}
      <h3 className={`font-serif text-ink ${compact ? "text-[17px]" : "text-[21px]"}`}>{title}</h3>
      {children && <p className="max-w-sm text-[12.5px] leading-relaxed text-muted">{children}</p>}
      {action && <div className="mt-1">{action}</div>}
    </div>
  );
}
