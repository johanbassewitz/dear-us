import { useState, type FormEvent } from "react";
import type { Profile } from "../../lib/api";
import { useSaveProfile } from "../../lib/queries";
import { localIsoDate } from "../../lib/dates";
import { Field } from "../../components/ui/Field";
import { Button } from "../../components/ui/Button";

interface Props {
  profile: Profile | undefined;
  submitLabel: string;
  onSaved?: () => void;
}

export function ProfileForm({ profile, submitLabel, onSaved }: Props) {
  const [myName, setMyName] = useState(profile?.me?.displayName ?? "");
  const [partnerName, setPartnerName] = useState(profile?.partner?.displayName ?? "");
  const [spaceName, setSpaceName] = useState(profile?.space?.name ?? "Our little world");
  const [startDate, setStartDate] = useState(profile?.space?.startDate ?? "");
  const [error, setError] = useState<string | null>(null);
  const save = useSaveProfile();

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    if (!myName.trim()) return setError("Tell us your name first ♡");
    try {
      await save.mutateAsync({
        myName,
        partnerName: partnerName.trim() || null,
        spaceName: spaceName.trim() || "Our little world",
        startDate: startDate || null,
      });
      onSaved?.();
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <form onSubmit={submit} className="flex flex-col gap-4">
      <div className="grid gap-4 sm:grid-cols-2">
        <Field label="Your name" value={myName} onChange={(e) => setMyName(e.target.value)} maxLength={60} autoFocus={!profile?.me} />
        <Field
          label="Your partner's name"
          value={partnerName}
          onChange={(e) => setPartnerName(e.target.value)}
          maxLength={60}
          hint="Optional. It updates once they join."
        />
      </div>
      <Field label="Name of your space" value={spaceName} onChange={(e) => setSpaceName(e.target.value)} maxLength={80} />
      <Field
        label="When did your story begin?"
        type="date"
        value={startDate}
        max={localIsoDate()}
        onChange={(e) => setStartDate(e.target.value)}
        hint="Optional. Used for “days together”."
      />
      {error && <p className="text-[12px] text-[#b0413e]">{error}</p>}
      <div>
        <Button type="submit" disabled={save.isPending}>
          {save.isPending ? "Saving…" : submitLabel}
        </Button>
      </div>
    </form>
  );
}
