import { NAV_GROUPS, SETTINGS_ITEM, type NavItem } from "./nav";
import { useNav } from "./navigation";
import { useProfile } from "../lib/queries";
import { Avatar } from "../components/ui/Avatar";

function NavButton({ item }: { item: NavItem }) {
  const page = useNav((s) => s.page);
  const go = useNav((s) => s.go);
  const active = page === item.id;
  const Icon = item.icon;
  return (
    <button
      onClick={() => go(item.id)}
      aria-current={active ? "page" : undefined}
      className={`flex w-full items-center gap-3 rounded-[11px] px-3 py-2.5 text-left text-[12.5px] font-semibold transition-colors duration-150 ${
        active
          ? "bg-accent-soft text-accent-deep shadow-[inset_3px_0_0_var(--accent)]"
          : "text-muted hover:bg-accent-soft/60 hover:text-accent-deep"
      }`}
    >
      <Icon size={17} strokeWidth={1.8} />
      <span className="flex-1">{item.label}</span>
    </button>
  );
}

export function Sidebar() {
  const { data: profile } = useProfile();
  const me = profile?.me?.displayName;
  const partner = profile?.partner?.displayName;

  return (
    <aside className="flex h-full w-[248px] shrink-0 flex-col overflow-y-auto border-r border-line bg-sidebar">
      <div className="flex items-center gap-3 px-7 pt-7 pb-6">
        <div className="grid h-10 w-10 -rotate-[4deg] place-items-center rounded-[14px] bg-accent font-serif text-2xl text-on-accent shadow-[0_8px_18px_color-mix(in_srgb,var(--accent)_20%,transparent)]">
          ♡
        </div>
        <div>
          <div className="font-serif text-[24px] leading-none font-bold tracking-[-0.04em]">dear us.</div>
          <div className="mt-1.5 text-[9px] font-extrabold tracking-[0.2em] text-faint uppercase">Our little universe</div>
        </div>
      </div>

      <div className="mx-4 mb-6 flex items-center gap-2.5 rounded-[14px] border border-line bg-[var(--couple)] px-3 py-3">
        <div className="flex items-center">
          <Avatar name={me} tone="a" />
          <span className="-ml-2">
            <Avatar name={partner} tone="b" />
          </span>
        </div>
        <div className="min-w-0">
          <strong className="block truncate text-[12px]">
            {me ? `${me}${partner ? ` & ${partner}` : ""}` : "You & your person"}
          </strong>
          <span className="block truncate text-[10.5px] text-accent">{profile?.space?.name ?? "A space for two"}</span>
        </div>
      </div>

      <nav aria-label="Main" className="flex flex-col gap-4">
        {NAV_GROUPS.map((g) => (
          <div key={g.name} className="px-4">
            <div className="mx-3 mb-2 text-[9px] font-extrabold tracking-[0.19em] text-faint uppercase">{g.name}</div>
            <div className="flex flex-col gap-0.5">
              {g.items.map((i) => (
                <NavButton key={i.id} item={i} />
              ))}
            </div>
          </div>
        ))}
      </nav>

      <div className="mt-auto px-4 pt-6 pb-5">
        <NavButton item={SETTINGS_ITEM} />
        <div className="mt-4 rounded-[14px] bg-gold px-4 py-3.5 text-center font-serif text-[13px] leading-snug text-muted italic">
          <span className="mb-1 block text-[17px] not-italic text-accent">✦</span>
          “The best part of life is sharing it with you.”
        </div>
      </div>
    </aside>
  );
}
