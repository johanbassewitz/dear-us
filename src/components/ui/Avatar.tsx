export function initialOf(name: string | null | undefined): string {
  const t = (name ?? "").trim();
  return t ? [...t][0].toUpperCase() : "♡";
}

export function Avatar({ name, tone = "a", size = 31 }: { name?: string | null; tone?: "a" | "b"; size?: number }) {
  const colors = tone === "a" ? "bg-[#dfb5a7] text-[#754a42]" : "bg-[#ccb8d1] text-[#604761]";
  return (
    <span
      className={`grid shrink-0 place-items-center rounded-full border-2 border-sidebar font-serif font-bold ${colors}`}
      style={{ width: size, height: size, fontSize: size * 0.42 }}
      aria-hidden="true"
    >
      {initialOf(name)}
    </span>
  );
}
