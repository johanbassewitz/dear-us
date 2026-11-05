import { api } from "../../lib/api";
import { invalidateBucket, invalidatePlanner } from "../../lib/queries";
import { toastError } from "../../components/ui/Toast";
import type { EditPrefill } from "../../app/navigation";

/**
 * One place that knows what to do after a NEW journal entry was written "for" something else.
 * Resolves to true when the link was made. Failures never lose the entry; they only tell the user.
 *
 * To support another kind: add it to `EditPrefill["linkTo"]` in app/navigation.ts and a case here.
 */
export async function linkNewEntry(linkTo: NonNullable<EditPrefill["linkTo"]>, entryId: string): Promise<boolean> {
  switch (linkTo.kind) {
    case "bucket":
      try {
        await api.bucketLinkEntry(linkTo.id, entryId);
        invalidateBucket();
        return true;
      } catch (e) {
        toastError(`The memory is saved, but it couldn't be linked: ${String(e)}`);
        return false;
      }
    case "date":
      try {
        await api.plannerLinkEntry(linkTo.id, entryId);
        invalidatePlanner();
        return true;
      } catch (e) {
        toastError(`The memory is saved, but it couldn't be linked: ${String(e)}`);
        return false;
      }
  }
}
