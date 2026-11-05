import { Hammer } from "lucide-react";
import { ALL_PAGES, type PageId } from "../../app/nav";
import { EmptyState } from "../../components/ui/EmptyState";

const COPY: Partial<Record<PageId, { eyebrow: string; title: string; text: string }>> = {
  journal: { eyebrow: "Keep the small things", title: "Journal", text: "Memories with photos, videos, reactions and comments." },
  timeline: { eyebrow: "Every chapter", title: "Our timeline", text: "Your whole story, milestone by milestone." },
  bucket: { eyebrow: "Dream a little bigger", title: "Bucket list", text: "Everything you want to do together, and everything you've already done." },
  planner: { eyebrow: "It doesn't need to be fancy", title: "Date planner", text: "Plan dates and shuffle through ideas." },
  letters: { eyebrow: "Words worth waiting for", title: "Love letters", text: "Letters, and time capsules that open on a chosen day." },
  questions: { eyebrow: "Put love into words", title: "Questions", text: "Gentle questions to answer together, at your own pace." },
  special: { eyebrow: "Mark your calendars", title: "Special dates", text: "Anniversaries, birthdays and countdowns." },
};

/** Honest placeholder for sections that aren't built yet — no mock controls. */
export function ComingSoonPage({ page }: { page: PageId }) {
  const c = COPY[page];
  const Icon = ALL_PAGES.find((p) => p.id === page)?.icon ?? Hammer;
  return (
    <div className="animate-fade-up">
      <div className="eyebrow">{c?.eyebrow}</div>
      <h1 className="page-title mt-2">{c?.title}</h1>
      <p className="mt-2 text-[13px] text-muted">{c?.text}</p>
      <div className="card mt-7">
        <EmptyState icon={<Icon size={22} />} title="This corner of your space is still being built">
          It's coming in an upcoming update. Everything you add elsewhere is already saved safely on this PC.
        </EmptyState>
      </div>
    </div>
  );
}
