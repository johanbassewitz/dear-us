import { useQuery } from "@tanstack/react-query";
import { call, isDesktop, type IdeaInput } from "../lib/api";

export interface CupidStatus {
  enabled: boolean;
  hasKey: boolean;
  privacyAck: boolean;
  model: string;
  dailyQuestion: boolean;
  dateIdeas: boolean;
  shareBucketTitles: boolean;
}
export interface ChatMessage {
  id: string;
  role: "user" | "model";
  content: string;
  createdAt: string;
}

export const cupidApi = {
  status: () => call<CupidStatus>("cupid_status"),
  setOption: (name: string, value: string) => call<CupidStatus>("cupid_set_option", { name, value }),
  setKey: (key: string) => call<CupidStatus>("cupid_set_key", { key }),
  removeKey: () => call<CupidStatus>("cupid_remove_key"),
  history: () => call<ChatMessage[]>("cupid_history"),
  clear: () => call<void>("cupid_clear_chat"),
  chat: (message: string) => call<ChatMessage>("cupid_chat", { message }),
  dateIdeas: (filters: Record<string, string>) => call<IdeaInput[]>("cupid_date_ideas", { filters }),
};

export const cupidKeys = { status: ["cupid", "status"] as const, history: ["cupid", "history"] as const };

export const useCupidStatus = () => useQuery({ queryKey: cupidKeys.status, queryFn: cupidApi.status, enabled: isDesktop });
