import { useState } from "react";
import { ArrowRight, HardDrive, Heart, Lock, Users } from "lucide-react";
import { ProfileForm } from "../settings/ProfileForm";
import { JoinForm } from "../sync/JoinForm";
import { RestorePanel } from "../backup/BackupSettings";
import { keys, queryClient } from "../../lib/queries";
import { Button } from "../../components/ui/Button";
import cupidHappy from "../../cupid/assets/cupid-happy.webp";

/** First run: create the space locally. Joining a partner arrives with sync. */
export function WelcomePage() {
  const [step, setStep] = useState<"hello" | "profile" | "join" | "restore">("hello");

  return (
    <div className="flex h-full items-center justify-center overflow-auto bg-bg p-6">
      <div className="animate-fade-up card w-full max-w-[620px] p-10">
        {step === "hello" ? (
          <div className="flex flex-col items-center text-center">
            <img src={cupidHappy} alt="" className="cupid-float mb-3 w-[190px] select-none" draggable={false} />
            <div className="eyebrow">Welcome to</div>
            <h1 className="page-title mt-2">dear us.</h1>
            <p className="mt-3 max-w-[420px] text-[13.5px] leading-relaxed text-muted">
              A private little home for your memories, letters, plans and dreams. Made for just the two of you.
            </p>
            <ul className="mt-6 grid w-full gap-2.5 text-left sm:grid-cols-3">
              {[
                { i: <HardDrive size={16} />, t: "Saved on your PC", d: "Your archive lives here, even offline." },
                { i: <Lock size={16} />, t: "Private", d: "No public profiles, no feeds. Just you two." },
                { i: <Heart size={16} />, t: "Yours to keep", d: "Your memories stay on your own computer." },
              ].map((x) => (
                <li key={x.t} className="rounded-xl bg-paper-2 p-3.5">
                  <span className="text-accent">{x.i}</span>
                  <strong className="mt-1.5 block text-[12.5px]">{x.t}</strong>
                  <span className="text-[11.5px] text-muted">{x.d}</span>
                </li>
              ))}
            </ul>
            <div className="mt-7 flex flex-wrap justify-center gap-3">
              <Button icon={<Heart size={15} />} onClick={() => setStep("profile")}>
                Create our space
              </Button>
              <Button variant="secondary" icon={<Users size={15} />} onClick={() => setStep("join")}>
                Join my partner
              </Button>
            </div>
            <button onClick={() => setStep("restore")} className="mt-4 text-[12px] text-muted underline-offset-2 hover:text-ink hover:underline">
              Restore from a backup instead
            </button>
          </div>
        ) : step === "restore" ? (
          <div>
            <div className="eyebrow">Moving to a new PC?</div>
            <h1 className="page-title mt-2 mb-1.5">Restore from a backup</h1>
            <p className="mb-6 text-[13px] text-muted">Choose your DearUsBackup folder. Every file is checked before it comes back. Afterwards, reconnect sync in Settings.</p>
            <RestorePanel onDone={() => queryClient.invalidateQueries({ queryKey: keys.profile })} />
            <button onClick={() => setStep("hello")} className="mt-5 inline-flex items-center gap-1 text-[12px] text-muted hover:text-ink">
              <ArrowRight size={13} className="rotate-180" /> Back
            </button>
          </div>
        ) : step === "join" ? (
          <div>
            <div className="eyebrow">Join your partner</div>
            <h1 className="page-title mt-2 mb-1.5">Paste your invite code</h1>
            <p className="mb-6 text-[13px] text-muted">Your partner makes it in Settings &amp; Sync. You'll get everything they've saved so far.</p>
            <JoinForm needName />
            <button onClick={() => setStep("hello")} className="mt-5 inline-flex items-center gap-1 text-[12px] text-muted hover:text-ink">
              <ArrowRight size={13} className="rotate-180" /> Back
            </button>
          </div>
        ) : (
          <div>
            <div className="eyebrow">Step 1 of 1</div>
            <h1 className="page-title mt-2 mb-1.5">Tell us about you two</h1>
            <p className="mb-6 text-[13px] text-muted">You can change all of this later in Settings.</p>
            {/* After save, the profile query updates and App switches to the main shell. */}
            <ProfileForm profile={undefined} submitLabel="Open our space" />
            <button onClick={() => setStep("hello")} className="mt-5 inline-flex items-center gap-1 text-[12px] text-muted hover:text-ink">
              <ArrowRight size={13} className="rotate-180" /> Back
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
