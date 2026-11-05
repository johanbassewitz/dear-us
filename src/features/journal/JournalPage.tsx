import { useNav } from "../../app/navigation";
import { JournalList } from "./JournalList";
import { EntryView } from "./EntryView";
import { EntryEditor } from "./EntryEditor";
import { TrashView } from "./TrashView";

export function JournalPage() {
  const view = useNav((s) => s.journal);
  switch (view.mode) {
    case "view":
      return <EntryView key={view.id} id={view.id} />;
    case "edit":
      return <EntryEditor key={view.id ?? "new"} id={view.id} prefill={view.prefill} />;
    case "trash":
      return <TrashView />;
    default:
      return <JournalList />;
  }
}
