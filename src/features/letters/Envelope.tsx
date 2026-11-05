/** A little sealed envelope drawn from theme tokens, so it looks right in every theme. */
export function Envelope({ className = "" }: { className?: string }) {
  return (
    <div className={`relative grid place-items-center ${className}`} aria-hidden="true">
      <div className="relative h-[115px] w-[176px] -rotate-[10deg] overflow-hidden rounded-[6px] bg-accent-soft shadow-[0_14px_22px_color-mix(in_srgb,var(--ink)_14%,transparent)] ring-1 ring-line">
        <svg viewBox="0 0 176 115" className="absolute inset-0 h-full w-full" fill="none">
          <path d="M0 0 L88 66 L176 0" stroke="var(--accent)" strokeOpacity="0.55" strokeWidth="1.5" />
          <path d="M0 115 L70 56 M176 115 L106 56" stroke="var(--accent)" strokeOpacity="0.3" strokeWidth="1.2" />
        </svg>
        <div className="absolute left-1/2 top-[58px] grid h-9 w-9 -translate-x-1/2 -translate-y-1/2 place-items-center rounded-full bg-accent text-on-accent shadow-md">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 21s-7.5-4.6-9.5-9.2C1.2 8.6 3 5.5 6.2 5.5c1.9 0 3.2 1 3.8 2.1.6-1.1 1.9-2.1 3.8-2.1 3.2 0 5 3.1 3.7 6.3C19.5 16.4 12 21 12 21z" />
          </svg>
        </div>
      </div>
      <span className="absolute bottom-2 font-hand text-[15px] text-accent-deep">sealed with love</span>
    </div>
  );
}
