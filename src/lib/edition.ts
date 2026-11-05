import { useAppInfo } from "./queries";

export type Edition = "personal" | "store";

/** Which edition this build is. The UI hides what doesn't apply (e.g. Supabase setup in the Store edition). */
export function useEdition(): Edition {
  return useAppInfo().data?.edition ?? "personal";
}

export const useIsStore = () => useEdition() === "store";
