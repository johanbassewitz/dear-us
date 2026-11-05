// Typed wrappers around the Rust commands. The UI never touches the DB or files directly.
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { previewInvoke } from "./previewBackend";

export interface AppInfo {
  version: string;
  dataDir: string;
  schemaVersion: number;
  /** "personal": own Supabase + Gemini key, GitHub updates. "store": Microsoft Store edition. */
  edition?: "personal" | "store";
}
export interface Person {
  id: string;
  displayName: string;
}
export interface Space {
  id: string;
  name: string;
  startDate: string | null;
}
export interface Profile {
  space: Space | null;
  me: Person | null;
  partner: Person | null;
}
export interface ProfileInput {
  myName: string;
  partnerName: string | null;
  spaceName: string;
  startDate: string | null;
}
export interface DashboardSummary {
  memories: number;
  bucketDone: number;
  bucketTotal: number;
}

export type PreviewState = "none" | "needed" | "image" | "video" | "failed";
export interface MediaRef {
  id: string;
  kind: "photo" | "video";
  mime: string;
  width: number | null;
  height: number | null;
  durationMs: number | null;
  takenAt: string | null;
  hasThumb: boolean;
  preview: PreviewState;
  availability: "local" | "pending" | "missing";
  originalName: string | null;
  size: number;
}
export type ScrapLayout = "auto" | "polaroid" | "collage" | "magazine" | "minimal";
export type ScrapFrame = "none" | "polaroid" | "tape" | "rounded" | "vintage";
export type ScrapPaper = "none" | "dots" | "lines" | "kraft";
export type ReactionType = "love" | "emotional" | "funny" | "adorable" | "special";
export interface ReactionCount {
  type: ReactionType;
  count: number;
  mine: boolean;
}
export interface EntryComment {
  id: string;
  entryId: string;
  authorId: string;
  authorName: string | null;
  body: string;
  createdAt: string;
  updatedAt: string;
  edited: boolean;
}
export interface EntrySummary {
  id: string;
  title: string;
  excerpt: string;
  entryDate: string;
  entryTime: string | null;
  location: string | null;
  tags: string[];
  authorId: string;
  authorName: string | null;
  highlight: boolean;
  cover: MediaRef | null;
  photoCount: number;
  videoCount: number;
  reactionCount: number;
  commentCount: number;
  updatedAt: string;
}
export interface EntryMedia extends MediaRef {
  caption: string | null;
}
export interface EntryDetail extends EntrySummary {
  bodyJson: string;
  media: EntryMedia[];
  coverMediaId: string | null;
  createdAt: string;
  layout: ScrapLayout;
  frame: ScrapFrame;
  paper: ScrapPaper;
}
export interface EntryInput {
  title: string;
  bodyJson: string;
  entryDate: string;
  entryTime: string | null;
  location: string | null;
  tags: string[];
  media: { id: string; caption: string | null }[];
  coverMediaId: string | null;
  highlight: boolean;
  layout: ScrapLayout;
  frame: ScrapFrame;
  paper: ScrapPaper;
}
export interface EntryQuery {
  text?: string;
  tag?: string;
  from?: string;
  to?: string;
  oldestFirst?: boolean;
  highlightsOnly?: boolean;
  limit?: number;
  offset?: number;
}
export interface TrashItem {
  id: string;
  title: string;
  entryDate: string;
  deletedAt: string;
  cover: MediaRef | null;
}
export interface TagCount {
  tag: string;
  count: number;
}
export interface ImportResult {
  path: string;
  media: MediaRef | null;
  error: string | null;
  duplicate: boolean;
}
export interface TimelineItem {
  kind: "milestone" | "entry";
  id: string;
  title: string;
  date: string;
  time: string | null;
  text: string;
  major: boolean;
  linkedEntryId: string | null;
  cover: MediaRef | null;
}
export interface MilestoneInput {
  title: string;
  date: string;
  description: string;
  coverMediaId: string | null;
  linkedEntryId: string | null;
}

export type Recurrence = "none" | "yearly" | "monthly";
export interface SpecialDate {
  id: string;
  title: string;
  date: string;
  time: string | null;
  description: string;
  recurrence: Recurrence;
  remindDaysBefore: number | null;
  linkedEntryId: string | null;
  linkedEntryTitle: string | null;
  createdAt: string;
  updatedAt: string;
}
export interface SpecialInput {
  title: string;
  date: string;
  time: string | null;
  description: string;
  recurrence: Recurrence;
  remindDaysBefore: number | null;
  linkedEntryId: string | null;
}

export type MoodId = "glowing" | "happy" | "calm" | "tired" | "low" | "stressed";
export interface CustomQuestion {
  id: string;
  text: string;
  category: string;
  createdAt: string;
}
export interface Answer {
  id: string;
  questionId: string;
  userId: string;
  mine: boolean;
  body: string;
  answeredForDate: string;
  createdAt: string;
  updatedAt: string;
}
export interface DailyQuestion {
  id: string;
  questionId: string;
}
export interface Mood {
  date: string;
  mood: MoodId;
  note: string;
}

export type BucketStatus ="want" | "planned" | "done";
export type BucketPriority = "low" | "medium" | "high";
export interface BucketItem {
  id: string;
  title: string;
  description: string;
  category: string;
  priority: BucketPriority;
  status: BucketStatus;
  targetDate: string | null;
  completedAt: string | null;
  linkedEntryId: string | null;
  linkedEntryTitle: string | null;
  coverMediaId: string | null;
  cover: MediaRef | null;
  media: MediaRef[];
  createdAt: string;
  updatedAt: string;
}
export interface BucketInput {
  title: string;
  description: string;
  category: string;
  priority: BucketPriority;
  status: BucketStatus;
  targetDate: string | null;
  coverMediaId: string | null;
  mediaIds: string[];
}

export type LetterKind = "letter" | "capsule";
export interface LetterInput {
  title: string;
  bodyJson: string;
  kind: LetterKind;
  /** RFC 3339 instant; capsules only. */
  unlockAt: string | null;
  showTitle: boolean;
  mediaIds: string[];
}
export interface LetterSummary {
  id: string;
  senderId: string;
  senderName: string;
  recipientId: string | null;
  recipientName: string | null;
  mine: boolean;
  /** null when the sender hid the title until the capsule opens. */
  title: string | null;
  kind: LetterKind;
  status: "draft" | "sent";
  sentAt: string | null;
  unlockAt: string | null;
  showTitle: boolean;
  lockKind: "tlock" | "none";
  sealed: boolean;
  needsUnlock: boolean;
  unlockedAt: string | null;
  excerpt: string;
  photoCount: number;
  cover: MediaRef | null;
  createdAt: string;
  updatedAt: string;
}
export interface LetterDetail extends LetterSummary {
  bodyJson: string;
  media: MediaRef[];
}

export type PlanStatus = "idea" | "planned" | "done" | "cancelled";
export interface ChecklistItem {
  id: string;
  text: string;
  done: boolean;
}
export interface PlanInput {
  title: string;
  description: string;
  date: string | null;
  time: string | null;
  location: string;
  budgetCents: number | null;
  currency: string;
  status: PlanStatus;
  checklist: ChecklistItem[];
  ideaRef: string | null;
}
export interface Plan extends PlanInput {
  id: string;
  linkedEntryId: string | null;
  linkedEntryTitle: string | null;
  createdAt: string;
  updatedAt: string;
}
export interface IdeaInput {
  title: string;
  description: string;
  category: string;
  budget: string;
  duration: string;
  setting: string;
  mood: string;
}
export interface CustomIdea extends IdeaInput {
  id: string;
}

/** True inside the real desktop app; false in a plain browser preview. */
export const isDesktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isDesktop) return invoke<T>(cmd, args);
  if (import.meta.env.DEV) return previewInvoke<T>(cmd, args);
  return Promise.reject(new Error("Dear Us must run as the desktop app."));
}

export type MediaVariant = "thumb" | "display" | "original";

/** URL for a media file, served by id through the app's private media:// protocol. */
export function mediaUrl(id: string, variant: MediaVariant): string | null {
  if (!isDesktop) return null;
  return convertFileSrc(`${id}/${variant}`, "media");
}

export const api = {
  appInfo: () => call<AppInfo>("app_info"),
  settingsGetAll: () => call<Record<string, string>>("settings_get_all"),
  settingsSet: (key: string, value: string) => call<void>("settings_set", { key, value }),
  profileGet: () => call<Profile>("profile_get"),
  profileSave: (input: ProfileInput) => call<Profile>("profile_save", { input }),
  dashboardSummary: () => call<DashboardSummary>("dashboard_summary"),
  openDataDir: () => call<void>("open_data_dir"),

  journalList: (query: EntryQuery = {}) => call<EntrySummary[]>("journal_list", { query }),
  journalGet: (id: string) => call<EntryDetail>("journal_get", { id }),
  journalCreate: (input: EntryInput) => call<EntryDetail>("journal_create", { input }),
  journalUpdate: (id: string, input: EntryInput) => call<EntryDetail>("journal_update", { id, input }),
  journalDelete: (id: string) => call<void>("journal_delete", { id }),
  journalRestore: (id: string) => call<EntryDetail>("journal_restore", { id }),
  journalPurge: (id: string) => call<void>("journal_purge", { id }),
  journalTrash: () => call<TrashItem[]>("journal_trash"),
  journalTags: () => call<TagCount[]>("journal_tags"),
  reactionList: (entryId: string) => call<ReactionCount[]>("reaction_list", { entryId }),
  reactionToggle: (entryId: string, kind: ReactionType) => call<ReactionCount[]>("reaction_toggle", { entryId, kind }),
  commentList: (entryId: string) => call<EntryComment[]>("comment_list", { entryId }),
  commentAdd: (entryId: string, body: string) => call<EntryComment>("comment_add", { entryId, body }),
  commentUpdate: (id: string, body: string) => call<EntryComment>("comment_update", { id, body }),
  commentDelete: (id: string) => call<void>("comment_delete", { id }),
  commentRestore: (id: string) => call<EntryComment>("comment_restore", { id }),
  journalOnThisDay: (today: string) => call<EntrySummary[]>("journal_on_this_day", { today }),

  timeline: (majorOnly: boolean) => call<TimelineItem[]>("timeline", { majorOnly }),
  milestoneCreate: (input: MilestoneInput) => call<string>("milestone_create", { input }),
  milestoneUpdate: (id: string, input: MilestoneInput) => call<void>("milestone_update", { id, input }),
  milestoneDelete: (id: string) => call<void>("milestone_delete", { id }),

  bucketList: () => call<BucketItem[]>("bucket_list"),
  bucketCreate: (input: BucketInput) => call<string>("bucket_create", { input }),
  bucketUpdate: (id: string, input: BucketInput) => call<void>("bucket_update", { id, input }),
  bucketSetStatus: (id: string, status: BucketStatus) => call<void>("bucket_set_status", { id, status }),
  bucketLinkEntry: (itemId: string, entryId: string | null) => call<void>("bucket_link_entry", { itemId, entryId }),
  bucketDelete: (id: string) => call<void>("bucket_delete", { id }),
  bucketRestore: (id: string) => call<void>("bucket_restore", { id }),
  bucketTrash: () => call<{ id: string; title: string; deletedAt: string }[]>("bucket_trash"),

  letterList: () => call<LetterSummary[]>("letter_list"),
  letterGet: (id: string) => call<LetterDetail>("letter_get", { id }),
  letterSaveDraft: (id: string | null, input: LetterInput) => call<string>("letter_save_draft", { id, input }),
  letterSend: (id: string) => call<void>("letter_send", { id }),
  letterDelete: (id: string) => call<void>("letter_delete", { id }),
  letterRestore: (id: string) => call<void>("letter_restore", { id }),
  /** Opens a capsule from your person once its moment has come (one public drand request). */
  letterUnlock: (id: string) => call<LetterDetail>("letter_unlock", { id }),

  specialList: () => call<SpecialDate[]>("special_list"),
  specialCreate: (input: SpecialInput) => call<string>("special_create", { input }),
  specialUpdate: (id: string, input: SpecialInput) => call<void>("special_update", { id, input }),
  specialDelete: (id: string) => call<void>("special_delete", { id }),
  specialRestore: (id: string) => call<void>("special_restore", { id }),

  plannerList: () => call<Plan[]>("planner_list"),
  plannerCreate: (input: PlanInput) => call<string>("planner_create", { input }),
  plannerUpdate: (id: string, input: PlanInput) => call<void>("planner_update", { id, input }),
  plannerSetStatus: (id: string, status: PlanStatus) => call<void>("planner_set_status", { id, status }),
  plannerToggleCheck: (id: string, itemId: string, done: boolean) => call<void>("planner_toggle_check", { id, itemId, done }),
  plannerLinkEntry: (id: string, entryId: string | null) => call<void>("planner_link_entry", { id, entryId }),
  plannerDelete: (id: string) => call<void>("planner_delete", { id }),
  plannerRestore: (id: string) => call<void>("planner_restore", { id }),
  ideaList: () => call<CustomIdea[]>("idea_list"),
  ideaCreate: (input: IdeaInput) => call<string>("idea_create", { input }),
  ideaDelete: (id: string) => call<void>("idea_delete", { id }),
  ideaRestore: (id: string) => call<void>("idea_restore", { id }),

  questionCustomList: () => call<CustomQuestion[]>("question_custom_list"),
  questionCustomCreate: (text: string, category: string) => call<CustomQuestion>("question_custom_create", { text, category }),
  questionCustomDelete: (id: string) => call<void>("question_custom_delete", { id }),
  questionCustomRestore: (id: string) => call<void>("question_custom_restore", { id }),
  questionAnswers: () => call<Answer[]>("question_answers"),
  questionAnswerSave: (questionId: string, body: string, date: string) =>
    call<Answer>("question_answer_save", { questionId, body, date }),
  questionAnswerDelete: (id: string) => call<void>("question_answer_delete", { id }),
  questionAnswerRestore: (id: string) => call<void>("question_answer_restore", { id }),
  questionDaily: (date: string, questionId: string) => call<DailyQuestion>("question_daily", { date, questionId }),
  moodList: (limit?: number) => call<Mood[]>("mood_list", { limit }),
  moodSave: (date: string, mood: MoodId, note: string) => call<Mood>("mood_save", { date, mood, note }),
  moodDelete: (date: string) => call<void>("mood_delete", { date }),

  mediaImport: (paths: string[]) => call<ImportResult[]>("media_import", { paths }),
  mediaCapabilities: () => call<{ converter: boolean }>("media_capabilities"),
  mediaInstallConverter: () => call<{ converter: boolean }>("media_install_converter"),
};
