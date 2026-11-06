import { useState } from "react";
import { Loader, Plus, Sparkles } from "lucide-react";
import { cupidApi, cupidKeys, useCupidStatus } from "./cupidApi";
import { api, type IdeaInput } from "../lib/api";
import { invalidatePlanner, queryClient } from "../lib/queries";
import { quotaError } from "../features/plan/planApi";
import { Button } from "../components/ui/Button";
import { toast, toastError } from "../components/ui/Toast";

/** "Ask Cupid" date ideas, using the current filters. Saved ideas join the couple's own list. */
export function CupidIdeas({ filters }: { filters: Record<string, string> }) {
  const { data: st } = useCupidStatus();
  const [ideas, setIdeas] = useState<IdeaInput[] | null>(null);
  const [busy, setBusy] = useState(false);
  if (!st?.enabled || !st.dateIdeas) return null;
  const ask = async () => {
    setBusy(true);
    try {
      setIdeas(await cupidApi.dateIdeas(Object.fromEntries(Object.entries(filters).filter(([, v]) => v))));
    } catch (e) {
      toastError(quotaError(e).message);
    } finally {
      queryClient.invalidateQueries({ queryKey: cupidKeys.status });
      setBusy(false);
    }
  };
  return (
    <div className="card mb-5 p-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <strong className="flex items-center gap-1.5 text-[13px]">
            <Sparkles size={14} className="text-accent" /> Ask Cupid
          </strong>
          <span className="text-[11.5px] text-muted">Fresh ideas from Cupid, using the filters above{st.shareBucketTitles ? " and your open bucket-list titles" : ""}.</span>
        </div>
        <Button size="sm" onClick={ask} disabled={busy} icon={busy ? <Loader size={13} className="animate-spin" /> : undefined}>
          {busy ? "Thinking…" : ideas ? "More ideas" : "Ask Cupid"}
        </Button>
      </div>
      {ideas && (
        <div className="mt-3 grid gap-3 md:grid-cols-3">
          {ideas.map((i, n) => (
            <div key={n} className="rounded-xl bg-paper-2 p-3.5">
              <strong className="block font-serif text-[16px]">{i.title}</strong>
              <p className="mt-1 text-[11.5px] leading-relaxed text-muted">{i.description}</p>
              <div className="mt-2 text-[10.5px] text-faint">
                {i.category.replace("-", " ")} · {i.budget} · {i.duration}
              </div>
              <Button
                className="mt-2"
                variant="secondary"
                size="sm"
                icon={<Plus size={12} />}
                onClick={async () => {
                  try {
                    await api.ideaCreate(i);
                    invalidatePlanner();
                    toast("Saved to your ideas ♡");
                    setIdeas((x) => x?.filter((_, k) => k !== n) ?? null);
                  } catch (e) {
                    toastError(String(e));
                  }
                }}
              >
                Save idea
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
