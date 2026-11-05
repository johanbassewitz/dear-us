import { call, type DailyQuestion } from "./api";
import { QUESTIONS } from "./questions";
import { QueryClient, useMutation, useQuery } from "@tanstack/react-query";
import { api, type SpecialInput, type PlanInput, type IdeaInput, type BucketInput,type EntryInput, type EntryQuery, type MilestoneInput, type ProfileInput, type LetterInput } from "./api";

export const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: Infinity, retry: 1, refetchOnWindowFocus: false } },
});

export const keys = {
  profile: ["profile"] as const,
  dashboard: ["dashboard"] as const,
  appInfo: ["appInfo"] as const,
  journal: ["journal"] as const, // prefix for everything journal-related
  entries: (q: EntryQuery) => ["journal", "list", q] as const,
  entry: (id: string) => ["journal", "entry", id] as const,
  reactions: (id: string) => ["journal", "reactions", id] as const,
  comments: (id: string) => ["journal", "comments", id] as const,
  trash: ["journal", "trash"] as const,
  tags: ["journal", "tags"] as const,
  onThisDay: (today: string) => ["journal", "onThisDay", today] as const,
  timeline: (major: boolean) => ["journal", "timeline", major] as const,
  capabilities: ["mediaCapabilities"] as const,
  bucket: ["bucket"] as const,
  special: ["special"] as const,
  planner: ["planner"] as const,
  ideas: ["planner", "ideas"] as const,
  questions: ["questions"] as const,
  letters: ["letters"] as const,
  letter: (id: string) => ["letters", "letter", id] as const,
};

/** After any special-date change. */
export function invalidateSpecial() {
  queryClient.invalidateQueries({ queryKey: keys.special });
  queryClient.invalidateQueries({ queryKey: keys.dashboard });
}

export const useSpecialDates = () => useQuery({ queryKey: keys.special, queryFn: api.specialList });

export const useSaveSpecialDate = () =>
  useMutation({
    mutationFn: ({ id, input }: { id: string | null; input: SpecialInput }) =>
      id ? api.specialUpdate(id, input).then(() => id) : api.specialCreate(input),
    onSuccess: invalidateSpecial,
  });

/** After any journal/timeline change: refresh lists, counts and timeline. */
export function invalidateJournal() {
  queryClient.invalidateQueries({ queryKey: keys.journal });
  queryClient.invalidateQueries({ queryKey: keys.dashboard });
}

/** After any bucket-list change: refresh the list and the dashboard counts. */
export function invalidateBucket() {
  queryClient.invalidateQueries({ queryKey: keys.bucket });
  queryClient.invalidateQueries({ queryKey: keys.dashboard });
}

export const useProfile = () => useQuery({ queryKey: keys.profile, queryFn: api.profileGet });
export const useDashboard = () => useQuery({ queryKey: keys.dashboard, queryFn: api.dashboardSummary });
export const useAppInfo = () => useQuery({ queryKey: keys.appInfo, queryFn: api.appInfo });
export const useCapabilities = () => useQuery({ queryKey: keys.capabilities, queryFn: api.mediaCapabilities });

export const useEntries = (q: EntryQuery) => useQuery({ queryKey: keys.entries(q), queryFn: () => api.journalList(q) });
export const useEntry = (id: string | null) =>
  useQuery({ queryKey: keys.entry(id ?? ""), queryFn: () => api.journalGet(id!), enabled: !!id });
export const useTrash = () => useQuery({ queryKey: keys.trash, queryFn: api.journalTrash });
export const useTags = () => useQuery({ queryKey: keys.tags, queryFn: api.journalTags });
export const useOnThisDay = (today: string) => useQuery({ queryKey: keys.onThisDay(today), queryFn: () => api.journalOnThisDay(today) });
export const useTimeline = (major: boolean) => useQuery({ queryKey: keys.timeline(major), queryFn: () => api.timeline(major) });

export const useSaveProfile = () =>
  useMutation({
    mutationFn: (input: ProfileInput) => api.profileSave(input),
    onSuccess: (profile) => queryClient.setQueryData(keys.profile, profile),
  });

export const useReactions = (entryId: string) =>
  useQuery({ queryKey: keys.reactions(entryId), queryFn: () => api.reactionList(entryId) });
export const useComments = (entryId: string) =>
  useQuery({ queryKey: keys.comments(entryId), queryFn: () => api.commentList(entryId) });

export const useSaveEntry = () =>
  useMutation({
    mutationFn: ({ id, input }: { id: string | null; input: EntryInput }) =>
      id ? api.journalUpdate(id, input) : api.journalCreate(input),
    onSuccess: (entry) => {
      queryClient.setQueryData(keys.entry(entry.id), entry);
      invalidateJournal();
    },
  });

export const useSaveMilestone = () =>
  useMutation({
    mutationFn: ({ id, input }: { id: string | null; input: MilestoneInput }) =>
      id ? api.milestoneUpdate(id, input).then(() => id) : api.milestoneCreate(input),
    onSuccess: invalidateJournal,
  });

export const useBucket = () => useQuery({ queryKey: keys.bucket, queryFn: api.bucketList });

export const useSaveBucketItem = () =>
  useMutation({
    mutationFn: ({ id, input }: { id: string | null; input: BucketInput }) =>
      id ? api.bucketUpdate(id, input).then(() => id) : api.bucketCreate(input),
    onSuccess: invalidateBucket,
  });

/** After any date-plan or custom-idea change. */
export function invalidatePlanner() {
  queryClient.invalidateQueries({ queryKey: keys.planner });
}

export const usePlans = () => useQuery({ queryKey: [...keys.planner, "plans"], queryFn: api.plannerList });
export const useCustomIdeas = () => useQuery({ queryKey: keys.ideas, queryFn: api.ideaList });

export const useSavePlan = () =>
  useMutation({
    mutationFn: ({ id, input }: { id: string | null; input: PlanInput }) =>
      id ? api.plannerUpdate(id, input).then(() => id) : api.plannerCreate(input),
    onSuccess: invalidatePlanner,
  });

export const useCreateIdea = () =>
  useMutation({ mutationFn: (input: IdeaInput) => api.ideaCreate(input), onSuccess: invalidatePlanner });

/** After any answer / custom-question change. */
export function invalidateQuestions() {
  queryClient.invalidateQueries({ queryKey: keys.questions });
}

export const useCustomQuestions = () => useQuery({ queryKey: [...keys.questions, "custom"], queryFn: api.questionCustomList });
export const useAnswers = () => useQuery({ queryKey: [...keys.questions, "answers"], queryFn: api.questionAnswers });
export const useMoods = () => useQuery({ queryKey: [...keys.questions, "moods"], queryFn: () => api.moodList(60) });
/** The day's question as first recorded on this device (or by the partner, once synced). */
export const useDailyQuestion = (date: string, questionId: string) =>
  useQuery({
    queryKey: [...keys.questions, "daily", date, questionId],
    queryFn: () => {
      // Cupid may choose from a shortlist starting at the fixed pick; otherwise the fixed pick.
      const at = Math.max(0, QUESTIONS.findIndex((q) => q.id === questionId));
      const candidates = Array.from({ length: 10 }, (_, k) => QUESTIONS[(at + k) % QUESTIONS.length]).map((q) => ({ id: q.id, text: q.text }));
      return call<DailyQuestion>("question_daily_smart", { date, fallbackId: questionId, candidates });
    },
  });

/** After any letter / capsule / draft change. */
export function invalidateLetters() {
  queryClient.invalidateQueries({ queryKey: keys.letters });
}

export const useLetters = () => useQuery({ queryKey: [...keys.letters, "list"], queryFn: api.letterList });
export const useLetter = (id: string | null) =>
  useQuery({ queryKey: keys.letter(id ?? ""), queryFn: () => api.letterGet(id!), enabled: !!id });

/** Saves (creates or updates) a draft. Drafts stay on this device until they're sent. */
export const useSaveLetterDraft = () =>
  useMutation({
    mutationFn: ({ id, input }: { id: string | null; input: LetterInput }) => api.letterSaveDraft(id, input),
    onSettled: invalidateLetters,
  });
