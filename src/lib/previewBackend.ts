// DEV-ONLY stand-in for the Rust core, used when the UI is opened in a plain browser
// (`npm run dev`) for design work and by UI tests. Data lives only in memory for the
// page session and the app shows a banner saying so. Photos/videos can't be imported
// here. Never used in the desktop app or production builds.

type Json = Record<string, unknown>;
interface Entry {
  id: string;
  title: string;
  bodyJson: string;
  entryDate: string;
  entryTime: string | null;
  location: string | null;
  tags: string[];
  highlight: boolean;
  layout: string;
  frame: string;
  paper: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
}
interface ReactionRow {
  id: string;
  entryId: string;
  userId: string;
  type: string;
  deletedAt: string | null;
}
interface CommentRow {
  id: string;
  entryId: string;
  authorId: string;
  body: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
}
interface Milestone {
  id: string;
  title: string;
  date: string;
  description: string;
  linkedEntryId: string | null;
  deletedAt: string | null;
}

interface Bucket {
  id: string;
  title: string;
  description: string;
  category: string;
  priority: string;
  status: string;
  targetDate: string | null;
  completedAt: string | null;
  linkedEntryId: string | null;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
}

const settings: Record<string, string> = {};
let profile: {
  space: { id: string; name: string; startDate: string | null } | null;
  me: { id: string; displayName: string } | null;
  partner: { id: string; displayName: string } | null;
} = { space: null, me: null, partner: null };
let entries: Entry[] = [];
let reactionRows: ReactionRow[] = [];
let commentRows: CommentRow[] = [];
let milestones: Milestone[] = [];
let bucket: Bucket[] = [];
interface LetterRow {
  id: string;
  senderId: string;
  recipientId: string | null;
  title: string;
  bodyJson: string;
  kind: "letter" | "capsule";
  status: "draft" | "sent";
  sentAt: string | null;
  unlockAt: string | null;
  showTitle: boolean;
  unlockedAt: string | null;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
  /** A capsule from the partner: what is sealed inside until it is opened. */
  sealed?: { title: string; bodyJson: string };
}
let letterRows: LetterRow[] = [];
interface Special {
  id: string;
  title: string;
  date: string;
  time: string | null;
  description: string;
  recurrence: string;
  remindDaysBefore: number | null;
  linkedEntryId: string | null;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
}
let specials: Special[] = [];
interface PlanRow {
  id: string;
  title: string;
  description: string;
  date: string | null;
  time: string | null;
  location: string;
  budgetCents: number | null;
  currency: string;
  status: string;
  checklist: { id: string; text: string; done: boolean }[];
  ideaRef: string | null;
  linkedEntryId: string | null;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
}
interface IdeaRow {
  id: string;
  title: string;
  description: string;
  category: string;
  budget: string;
  duration: string;
  setting: string;
  mood: string;
  deletedAt: string | null;
}
let plans: PlanRow[] = [];
let ideas: IdeaRow[] = [];
interface CustomQ { id: string; text: string; category: string; createdAt: string; deletedAt: string | null }
interface AnswerRow { id: string; questionId: string; userId: string; body: string; answeredForDate: string; createdAt: string; updatedAt: string; deletedAt: string | null }
const CATEGORY_IDS = ["favorite-moments", "appreciation", "communication", "plans", "goals", "reflection"];
const MOOD_IDS = ["glowing", "happy", "calm", "tired", "low", "stressed"];
let customQs: CustomQ[] = [];
let answerRows: AnswerRow[] = [];
const dailyQs = new Map<string, string>();
const moodRows = new Map<string, { date: string; mood: string; note: string }>();
let seq = 0;
const newId = () => `00000000-0000-7000-8000-${String(++seq).padStart(12, "0")}`;
const now = () => new Date().toISOString();

const params = typeof location !== "undefined" ? new URLSearchParams(location.search) : new URLSearchParams();
// `?space` skips the welcome screen in the browser preview (design checks only).
if (params.has("space")) {
  profile = {
    space: { id: "preview-space", name: "Our little world", startDate: "2024-07-03" },
    me: { id: "preview-me", displayName: "Johan" },
    partner: null,
  };
  if (params.get("theme")) settings["ui.theme"] = JSON.stringify(params.get("theme"));
  if (params.get("page")) settings["ui.lastPage"] = JSON.stringify(params.get("page"));
}
if (params.has("demo")) {
  const doc = (t: string) => JSON.stringify({ type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text: t }] }] });
  const y = new Date().getFullYear();
  const md = new Date().toISOString().slice(5, 10);
  const mk = (title: string, date: string, text: string, tags: string[], highlight = false): Entry => ({
    id: newId(), title, bodyJson: doc(text), entryDate: date, entryTime: null, location: "Beirut",
    tags, highlight, layout: "auto", frame: "none", paper: "none", createdAt: now(), updatedAt: now(), deletedAt: null,
  });
  entries = [
    mk("Sunset picnic on the rooftop", `${y}-${md}`, "We brought too many snacks and stayed until the stars came out.", ["picnic", "summer"]),
    mk("Our first road trip", `${y - 1}-${md}`, "Windows down, a terrible playlist, and the best day.", ["trip"], true),
    mk("Cooking night", `${y}-01-12`, "We tried making pasta from scratch. Flour everywhere.", ["home", "food"]),
  ];
  reactionRows = [
    { id: newId(), entryId: entries[0].id, userId: "preview-partner", type: "love", deletedAt: null },
    { id: newId(), entryId: entries[0].id, userId: "preview-me", type: "adorable", deletedAt: null },
    { id: newId(), entryId: entries[1].id, userId: "preview-partner", type: "special", deletedAt: null },
  ];
  commentRows = [
    { id: newId(), entryId: entries[0].id, authorId: "preview-partner", body: "I still think about those snacks. Best evening.", createdAt: now(), updatedAt: now(), deletedAt: null },
  ];
  const bk = (title: string, category: string, priority: string, status: string, targetDate: string | null = null): Bucket => ({
    id: newId(), title, description: "", category, priority, status, targetDate,
    completedAt: status === "done" ? now() : null, linkedEntryId: null, createdAt: now(), updatedAt: now(), deletedAt: null,
  });
  bucket = [
    bk("See the northern lights", "Travel", "high", "want"),
    bk("Cook a five-course dinner together", "Food", "medium", "want"),
    bk("Weekend in the mountains", "Adventure", "high", "planned", `${y + 1}-02-14`),
    bk("Build a blanket fort", "At home", "low", "done"),
    bk("Learn to slow dance", "Learning", "medium", "done"),
  ];
  const plus = (days: number) => {
    const d = new Date();
    d.setDate(d.getDate() + days);
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  };
  const sd = (title: string, date: string, recurrence: string, remindDaysBefore: number | null = null): Special => ({
    id: newId(), title, date, time: null, description: "", recurrence, remindDaysBefore, linkedEntryId: null, createdAt: now(), updatedAt: now(), deletedAt: null,
  });
  specials = [
    sd("Sam's birthday", `${y - 28}-${plus(23).slice(5)}`, "yearly", 7),
    sd("Weekend away", plus(40), "none", 3),
    sd("Our first apartment", "2023-09-01", "none"),
  ];
  const pl = (title: string, date: string | null, status: string, o: Partial<PlanRow> = {}): PlanRow => ({
    id: newId(), title, description: "", date, time: null, location: "", budgetCents: null, currency: "USD", status,
    checklist: [], ideaRef: null, linkedEntryId: null, createdAt: now(), updatedAt: now(), deletedAt: null, ...o,
  });
  plans = [
    pl("Sunset picnic on the rooftop", plus(6), "planned", {
      time: "18:30", location: "Our rooftop", budgetCents: 3000,
      checklist: [{ id: newId(), text: "Blanket", done: true }, { id: newId(), text: "Snacks and lemonade", done: false }],
    }),
    pl("Pottery class", plus(25), "planned", { budgetCents: 8000 }),
    pl("Stargazing night", null, "idea"),
  ];
  const mineId = "preview-me";
  const ans = (questionId: string, userId: string, body: string, answeredForDate: string): AnswerRow => ({
    id: newId(), questionId, userId, body, answeredForDate, createdAt: now(), updatedAt: now(), deletedAt: null,
  });
  customQs = [{ id: newId(), text: "What song should be ours?", category: "favorite-moments", createdAt: now(), deletedAt: null }];
  answerRows = [
    ans("favorite-moments-02", mineId, "The night we couldn't stop laughing at the terrible movie.", plus(-1)),
    ans("favorite-moments-02", "preview-partner", "Your impression of the waiter. I still smile about it.", plus(-1)),
    ans("appreciation-01", mineId, "How you make the mornings feel slow and kind.", plus(-9)),
  ];
  moodRows.set(plus(-1), { date: plus(-1), mood: "calm", note: "A quiet, good day." });
  const lt = (o: Partial<LetterRow> & Pick<LetterRow, "title" | "kind" | "status">, text: string): LetterRow => ({
    id: newId(), senderId: "preview-me", recipientId: "preview-partner", bodyJson: doc(text), sentAt: o.status === "sent" ? now() : null,
    unlockAt: null, showTitle: true, unlockedAt: null, createdAt: now(), updatedAt: now(), deletedAt: null, ...o,
  });
  const inDays = (d: number) => new Date(Date.now() + d * 86_400_000).toISOString();
  letterRows = [
    lt({ title: "For the rainy days", kind: "letter", status: "sent" }, "When the weather is grey, remember how warm you make every room."),
    lt({ title: "Open on our anniversary", kind: "capsule", status: "sent", unlockAt: inDays(120) }, "By the time you read this, we will have a whole new year behind us."),
    lt({ title: "Something I never said out loud", kind: "letter", status: "draft" }, "I have been meaning to tell you that"),
    lt({
      title: "", kind: "capsule", status: "sent", senderId: "preview-partner", recipientId: "preview-me", showTitle: false,
      unlockAt: inDays(45), bodyJson: "", sealed: { title: "A little something", bodyJson: doc("Happy birthday. Look up.") },
    }, ""),
  ];
  milestones = [{ id: newId(), title: "The day we met", date: "2024-07-03", description: "A rainy café, one umbrella.", linkedEntryId: null, deletedAt: null }];
}

// ---- letters & time capsules ----
function personName(id: string | null): string | null {
  if (!id) return null;
  if (id === "preview-me") return profile.me?.displayName ?? "You";
  return profile.partner?.displayName ?? "Sam";
}
function letterMoment(l: LetterRow) {
  return !!l.unlockAt && new Date(l.unlockAt).getTime() <= Date.now();
}
function letterSummary(l: LetterRow) {
  const mine = l.senderId === "preview-me";
  const capsule = l.kind === "capsule";
  const sent = l.status === "sent";
  const passed = letterMoment(l);
  const opened = mine || !capsule || !!l.unlockedAt;
  const recipientId = l.recipientId ?? (mine && !sent && profile.partner ? "preview-partner" : null);
  return {
    id: l.id, senderId: l.senderId, senderName: personName(l.senderId) ?? "", recipientId, recipientName: personName(recipientId),
    mine, title: opened || l.showTitle ? l.title : null, kind: l.kind, status: l.status, sentAt: l.sentAt, unlockAt: l.unlockAt,
    showTitle: l.showTitle, lockKind: capsule && sent ? "tlock" : "none", sealed: capsule && sent && !passed && !l.unlockedAt,
    needsUnlock: !mine && capsule && sent && passed && !opened, unlockedAt: l.unlockedAt,
    excerpt: opened ? excerpt(l.bodyJson).slice(0, 180) : "", photoCount: 0, cover: null, createdAt: l.createdAt, updatedAt: l.updatedAt,
  };
}
function findLetter(id: unknown, includeDeleted = false) {
  const l = letterRows.find((x) => x.id === id && (includeDeleted ? !!x.deletedAt : !x.deletedAt));
  if (!l) throw new Error(includeDeleted ? "nothing to restore" : "this letter doesn't exist anymore");
  return l;
}
function letterFields(i: Json) {
  const title = String(i.title ?? "").trim();
  if (title.length > 200) throw new Error("the title is too long (200 characters max)");
  const kind = String(i.kind);
  if (kind !== "letter" && kind !== "capsule") throw new Error("pick a letter or a time capsule");
  let unlockAt: string | null = null;
  if (kind === "capsule" && i.unlockAt) {
    const t = new Date(String(i.unlockAt));
    if (Number.isNaN(t.getTime())) throw new Error("pick a valid date and time");
    unlockAt = t.toISOString();
  }
  return { title, bodyJson: String(i.bodyJson ?? ""), kind: kind as "letter" | "capsule", unlockAt, showTitle: kind === "letter" || i.showTitle !== false };
}

function excerpt(bodyJson: string): string {
  try {
    const out: string[] = [];
    const walk = (n: Json) => {
      if (typeof n.text === "string") out.push(n.text);
      (n.content as Json[] | undefined)?.forEach(walk);
    };
    walk(JSON.parse(bodyJson));
    return out.join(" ").slice(0, 240);
  } catch {
    return "";
  }
}

function summary(e: Entry) {
  return {
    id: e.id, title: e.title, excerpt: excerpt(e.bodyJson), entryDate: e.entryDate, entryTime: e.entryTime,
    location: e.location, tags: e.tags, authorId: "preview-me", authorName: profile.me?.displayName ?? null,
    highlight: e.highlight, cover: null, photoCount: 0, videoCount: 0, updatedAt: e.updatedAt,
    reactionCount: reactionRows.filter((x) => x.entryId === e.id && !x.deletedAt).length,
    commentCount: commentRows.filter((x) => x.entryId === e.id && !x.deletedAt).length,
  };
}
const detail = (e: Entry) => ({ ...summary(e), bodyJson: e.bodyJson, media: [], coverMediaId: null, createdAt: e.createdAt, layout: e.layout, frame: e.frame, paper: e.paper });
const REACTION_TYPES = ["love", "emotional", "funny", "adorable", "special"];
const meId = () => profile.me?.id ?? "preview-me";
const reactionView = (entryId: string) =>
  REACTION_TYPES.map((type) => {
    const rows = reactionRows.filter((x) => x.entryId === entryId && x.type === type && !x.deletedAt);
    return { type, count: rows.length, mine: rows.some((x) => x.userId === meId()) };
  });
const commentView = (c: CommentRow) => ({
  ...c,
  authorName: c.authorId === meId() ? (profile.me?.displayName ?? null) : (profile.partner?.displayName ?? "Your person"),
  edited: c.updatedAt !== c.createdAt,
});
function ownComment(id: unknown, deleted = false) {
  const c = commentRows.find((x) => x.id === id && !!x.deletedAt === deleted);
  if (!c) throw new Error(deleted ? "nothing to restore" : "this comment doesn't exist anymore");
  if (c.authorId !== meId()) throw new Error("You can only change your own comments");
  return c;
}
function commentBody(v: unknown) {
  const body = String(v ?? "").trim();
  if (!body) throw new Error("Write a few words first ♡");
  if (body.length > 2000) throw new Error("that comment is too long (2000 characters max)");
  return body;
}
const pickStyle = (v: unknown, allowed: string[], fallback: string, what: string) => {
  const s = v === undefined || v === null ? fallback : String(v);
  if (!allowed.includes(s)) throw new Error(`unknown ${what} style`);
  return s;
};
const live = () => entries.filter((e) => !e.deletedAt);
const find = (id: unknown) => {
  const e = entries.find((x) => x.id === id);
  if (!e) throw new Error("this memory doesn't exist anymore");
  return e;
};

function specialFields(i: Json) {
  const title = String(i.title ?? "").trim();
  if (!title) throw new Error("A special date needs a title (200 characters max)");
  const date = String(i.date ?? "").trim();
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) throw new Error("date must be YYYY-MM-DD");
  const recurrence = String(i.recurrence);
  if (!["none", "yearly", "monthly"].includes(recurrence)) throw new Error("pick how often it repeats: never, yearly or monthly");
  const remind = i.remindDaysBefore === null || i.remindDaysBefore === undefined ? null : Number(i.remindDaysBefore);
  if (remind !== null && !(Number.isInteger(remind) && remind >= 0 && remind <= 60)) throw new Error("reminders can be 0 to 60 days before");
  const linked = (i.linkedEntryId as string) || null;
  if (linked) find(linked);
  return { title, date, time: (i.time as string) || null, description: String(i.description ?? "").trim(), recurrence, remindDaysBefore: remind, linkedEntryId: linked };
}

const liveBucket =() => bucket.filter((b) => !b.deletedAt);
const findBucket = (id: unknown) => {
  const b = bucket.find((x) => x.id === id && !x.deletedAt);
  if (!b) throw new Error("this dream doesn't exist anymore");
  return b;
};
const bucketView = (b: Bucket) => {
  const linked = b.linkedEntryId ? live().find((e) => e.id === b.linkedEntryId) : undefined;
  return { ...b, linkedEntryId: linked ? b.linkedEntryId : null, linkedEntryTitle: linked?.title ?? null, coverMediaId: null, cover: null, media: [] };
};
function bucketFields(i: Json, prev?: Bucket) {
  const title = String(i.title ?? "").trim();
  if (!title) throw new Error("Give this dream a title (200 characters max)");
  if (!["low", "medium", "high"].includes(String(i.priority))) throw new Error("pick a priority: low, medium or high");
  const status = String(i.status);
  if (!["want", "planned", "done"].includes(status)) throw new Error("pick Dreaming, Planned or Done");
  return {
    title,
    description: String(i.description ?? "").trim(),
    category: String(i.category ?? "").trim(),
    priority: String(i.priority),
    status,
    targetDate: (i.targetDate as string) || null,
    completedAt: status !== "done" ? null : prev?.status === "done" && prev.completedAt ? prev.completedAt : now(),
  };
}

const livePlans = () => plans.filter((p) => !p.deletedAt);
const findPlan = (id: unknown) => {
  const p = plans.find((x) => x.id === id && !x.deletedAt);
  if (!p) throw new Error("this date doesn't exist anymore");
  return p;
};
function planFields(i: Json) {
  const title = String(i.title ?? "").trim();
  if (!title) throw new Error("Give this date a name (200 characters max)");
  const status = String(i.status);
  if (!["idea", "planned", "done", "cancelled"].includes(status)) throw new Error("pick Idea, Planned, Done or Cancelled");
  const date = String(i.date ?? "").trim() || null;
  if (status === "planned" && !date) throw new Error("pick a day for a planned date, or keep it as an idea");
  const budget = i.budgetCents == null ? null : Number(i.budgetCents);
  if (budget !== null && !(budget >= 0)) throw new Error("the budget must be a positive amount");
  const checklist = ((i.checklist as { id?: string; text: string; done?: boolean }[]) ?? [])
    .filter((c) => c.text.trim())
    .map((c) => ({ id: c.id || newId(), text: c.text.trim(), done: !!c.done }));
  return {
    title, status, date, budgetCents: budget, checklist,
    description: String(i.description ?? "").trim(),
    time: (i.time as string) || null,
    location: String(i.location ?? "").trim(),
    currency: String(i.currency || "USD").toUpperCase(),
    ideaRef: (i.ideaRef as string) || null,
  };
}
const planView = (p: PlanRow) => {
  const { deletedAt: _d, ...rest } = p;
  const linked = p.linkedEntryId ? live().find((e) => e.id === p.linkedEntryId) : undefined;
  return { ...rest, checklist: rest.checklist.map((c) => ({ ...c })), linkedEntryId: linked ? p.linkedEntryId : null, linkedEntryTitle: linked?.title ?? null };
};

function fromInput(i: Json): Omit<Entry, "id" | "createdAt" | "updatedAt" | "deletedAt"> {
  const title = String(i.title ?? "").trim();
  if (!title) throw new Error("Give this memory a title ♡");
  return {
    title,
    bodyJson: String(i.bodyJson ?? ""),
    entryDate: String(i.entryDate),
    entryTime: (i.entryTime as string) || null,
    location: ((i.location as string) || "").trim() || null,
    tags: [...new Set(((i.tags as string[]) ?? []).map((t) => t.trim().toLowerCase()).filter(Boolean))],
    highlight: !!i.highlight,
    layout: pickStyle(i.layout, ["auto", "polaroid", "collage", "magazine", "minimal"], "auto", "layout"),
    frame: pickStyle(i.frame, ["none", "polaroid", "tape", "rounded", "vintage"], "none", "frame"),
    paper: pickStyle(i.paper, ["none", "dots", "lines", "kraft"], "none", "paper"),
  };
}

export async function previewInvoke<T>(cmd: string, args: Json = {}): Promise<T> {
  const r = (v: unknown) => v as T;
  switch (cmd) {
    case "app_info":
      return r({ version: "dev-preview", dataDir: "(browser preview: nothing saved)", schemaVersion: 2 });
    case "settings_get_all":
      return r({ ...settings });
    case "settings_set":
      settings[args.key as string] = args.value as string;
      return r(undefined);
    case "profile_get":
      return r(profile);
    case "profile_save": {
      const i = args.input as { myName: string; partnerName: string | null; spaceName: string; startDate: string | null };
      profile = {
        space: { id: "preview-space", name: i.spaceName.trim(), startDate: i.startDate || null },
        me: { id: "preview-me", displayName: i.myName.trim() },
        partner: i.partnerName?.trim() ? { id: "preview-partner", displayName: i.partnerName.trim() } : profile.partner,
      };
      return r(profile);
    }
    case "dashboard_summary":
      return r({ memories: live().length, bucketDone: liveBucket().filter((b) => b.status === "done").length, bucketTotal: liveBucket().length });
    case "journal_list": {
      const q = (args.query ?? {}) as Json;
      let list = live();
      const text = String(q.text ?? "").trim().toLowerCase();
      if (text) list = list.filter((e) => `${e.title} ${excerpt(e.bodyJson)} ${e.tags.join(" ")} ${e.location}`.toLowerCase().includes(text));
      if (q.tag) list = list.filter((e) => e.tags.includes(String(q.tag).toLowerCase()));
      if (q.from) list = list.filter((e) => e.entryDate >= String(q.from));
      if (q.to) list = list.filter((e) => e.entryDate <= String(q.to));
      if (q.highlightsOnly) list = list.filter((e) => e.highlight);
      list.sort((a, b) => b.entryDate.localeCompare(a.entryDate));
      if (q.oldestFirst) list.reverse();
      return r(list.slice(0, Number(q.limit ?? 500)).map(summary));
    }
    case "journal_get":
      return r(detail(find(args.id)));
    case "journal_create": {
      const e: Entry = { ...fromInput(args.input as Json), id: newId(), createdAt: now(), updatedAt: now(), deletedAt: null };
      entries.push(e);
      return r(detail(e));
    }
    case "journal_update": {
      const e = find(args.id);
      Object.assign(e, fromInput(args.input as Json), { updatedAt: now() });
      return r(detail(e));
    }
    case "journal_delete":
      find(args.id).deletedAt = now();
      return r(undefined);
    case "journal_restore": {
      const e = find(args.id);
      e.deletedAt = null;
      return r(detail(e));
    }
    case "reaction_list":
      return r(reactionView(String(args.entryId)));
    case "reaction_toggle": {
      const entryId = String(args.entryId);
      const kind = String(args.kind);
      if (!REACTION_TYPES.includes(kind)) throw new Error("unknown reaction");
      find(entryId);
      const mine = reactionRows.find((x) => x.entryId === entryId && x.userId === meId() && x.type === kind && !x.deletedAt);
      if (mine) mine.deletedAt = now();
      else reactionRows.push({ id: newId(), entryId, userId: meId(), type: kind, deletedAt: null });
      return r(reactionView(entryId));
    }
    case "comment_list":
      return r(commentRows.filter((c) => c.entryId === args.entryId && !c.deletedAt).map(commentView));
    case "comment_add": {
      find(args.entryId);
      const c: CommentRow = { id: newId(), entryId: String(args.entryId), authorId: meId(), body: commentBody(args.body), createdAt: now(), updatedAt: now(), deletedAt: null };
      commentRows.push(c);
      return r(commentView(c));
    }
    case "comment_update": {
      const body = commentBody(args.body);
      const c = ownComment(args.id);
      if (c.body !== body) Object.assign(c, { body, updatedAt: new Date(Date.now() + 1).toISOString() });
      return r(commentView(c));
    }
    case "comment_delete":
      ownComment(args.id).deletedAt = now();
      return r(undefined);
    case "comment_restore": {
      const c = ownComment(args.id, true);
      c.deletedAt = null;
      return r(commentView(c));
    }
    case "journal_purge":
      entries = entries.filter((e) => e.id !== args.id);
      reactionRows = reactionRows.filter((x) => x.entryId !== args.id);
      commentRows = commentRows.filter((x) => x.entryId !== args.id);
      return r(undefined);
    case "journal_trash":
      return r(entries.filter((e) => e.deletedAt).map((e) => ({ id: e.id, title: e.title, entryDate: e.entryDate, deletedAt: e.deletedAt, cover: null })));
    case "journal_tags": {
      const counts = new Map<string, number>();
      live().forEach((e) => e.tags.forEach((t) => counts.set(t, (counts.get(t) ?? 0) + 1)));
      return r([...counts].map(([tag, count]) => ({ tag, count })).sort((a, b) => b.count - a.count));
    }
    case "journal_on_this_day": {
      const today = String(args.today);
      return r(live().filter((e) => e.entryDate.slice(5) === today.slice(5) && e.entryDate.slice(0, 4) < today.slice(0, 4)).map(summary));
    }
    case "timeline": {
      const major = !!args.majorOnly;
      const items = [
        ...milestones.filter((m) => !m.deletedAt).map((m) => ({ kind: "milestone", id: m.id, title: m.title, date: m.date, time: null, text: m.description, major: true, linkedEntryId: m.linkedEntryId, cover: null })),
        ...live().filter((e) => !major || e.highlight).map((e) => ({ kind: "entry", id: e.id, title: e.title, date: e.entryDate, time: e.entryTime, text: excerpt(e.bodyJson), major: e.highlight, linkedEntryId: e.id, cover: null })),
      ];
      return r(items.sort((a, b) => b.date.localeCompare(a.date)));
    }
    case "milestone_create": {
      const i = args.input as Json;
      if (!String(i.title ?? "").trim()) throw new Error("A milestone needs a title");
      const id = newId();
      milestones.push({ id, title: String(i.title).trim(), date: String(i.date), description: String(i.description ?? ""), linkedEntryId: (i.linkedEntryId as string) || null, deletedAt: null });
      return r(id);
    }
    case "milestone_update": {
      const m = milestones.find((x) => x.id === args.id)!;
      const i = args.input as Json;
      Object.assign(m, { title: String(i.title).trim(), date: String(i.date), description: String(i.description ?? ""), linkedEntryId: (i.linkedEntryId as string) || null });
      return r(undefined);
    }
    case "milestone_delete":
      milestones.find((x) => x.id === args.id)!.deletedAt = now();
      return r(undefined);
    case "bucket_list":
      return r([...liveBucket()].reverse().map(bucketView));
    case "bucket_create": {
      const b: Bucket = { ...bucketFields(args.input as Json), id: newId(), linkedEntryId: null, createdAt: now(), updatedAt: now(), deletedAt: null };
      bucket.push(b);
      return r(b.id);
    }
    case "bucket_update": {
      const b = findBucket(args.id);
      Object.assign(b, bucketFields(args.input as Json, b), { updatedAt: now() });
      return r(undefined);
    }
    case "bucket_set_status": {
      const b = findBucket(args.id);
      const status = String(args.status);
      if (!["want", "planned", "done"].includes(status)) throw new Error("pick Dreaming, Planned or Done");
      b.completedAt = status !== "done" ? null : b.status === "done" && b.completedAt ? b.completedAt : now();
      b.status = status;
      b.updatedAt = now();
      return r(undefined);
    }
    case "bucket_link_entry": {
      const b = findBucket(args.itemId);
      const eid = (args.entryId as string) || null;
      if (eid) find(eid);
      b.linkedEntryId = eid;
      return r(undefined);
    }
    case "bucket_delete":
      findBucket(args.id).deletedAt = now();
      return r(undefined);
    case "bucket_restore": {
      const b = bucket.find((x) => x.id === args.id && x.deletedAt);
      if (!b) throw new Error("nothing to restore");
      b.deletedAt = null;
      return r(undefined);
    }
    case "bucket_trash":
      return r(bucket.filter((b) => b.deletedAt).map((b) => ({ id: b.id, title: b.title, deletedAt: b.deletedAt })));
    case "special_list":
      return r(
        specials
          .filter((s) => !s.deletedAt)
          .sort((a, b) => a.date.localeCompare(b.date))
          .map(({ deletedAt: _d, ...s }) => ({ ...s, linkedEntryTitle: live().find((e) => e.id === s.linkedEntryId)?.title ?? null })),
      );
    case "special_create": {
      const s: Special = { ...specialFields(args.input as Json), id: newId(), createdAt: now(), updatedAt: now(), deletedAt: null };
      specials.push(s);
      return r(s.id);
    }
    case "special_update": {
      const s = specials.find((x) => x.id === args.id && !x.deletedAt);
      if (!s) throw new Error("this special date doesn't exist anymore");
      Object.assign(s, specialFields(args.input as Json), { updatedAt: now() });
      return r(undefined);
    }
    case "special_delete": {
      const s = specials.find((x) => x.id === args.id && !x.deletedAt);
      if (!s) throw new Error("this special date doesn't exist anymore");
      s.deletedAt = now();
      return r(undefined);
    }
    case "special_restore": {
      const s = specials.find((x) => x.id === args.id && x.deletedAt);
      if (!s) throw new Error("nothing to restore");
      s.deletedAt = null;
      return r(undefined);
    }
    case "planner_list":
      return r(
        [...livePlans()]
          .sort((a, b) => (a.date ?? "9999").localeCompare(b.date ?? "9999") || (a.time ?? "99").localeCompare(b.time ?? "99"))
          .map(planView),
      );
    case "planner_create": {
      const p: PlanRow = { ...planFields(args.input as Json), id: newId(), linkedEntryId: null, createdAt: now(), updatedAt: now(), deletedAt: null };
      plans.push(p);
      return r(p.id);
    }
    case "planner_update": {
      const p = findPlan(args.id);
      Object.assign(p, planFields(args.input as Json), { updatedAt: now() });
      return r(undefined);
    }
    case "planner_set_status": {
      const p = findPlan(args.id);
      const status = String(args.status);
      if (!["idea", "planned", "done", "cancelled"].includes(status)) throw new Error("pick Idea, Planned, Done or Cancelled");
      if (status === "planned" && !p.date) throw new Error("pick a day for a planned date, or keep it as an idea");
      p.status = status;
      return r(undefined);
    }
    case "planner_toggle_check": {
      const c = findPlan(args.id).checklist.find((x) => x.id === args.itemId);
      if (!c) throw new Error("that checklist item doesn't exist anymore");
      c.done = !!args.done;
      return r(undefined);
    }
    case "planner_link_entry": {
      const p = findPlan(args.id);
      const eid = (args.entryId as string) || null;
      if (eid) find(eid);
      p.linkedEntryId = eid;
      return r(undefined);
    }
    case "planner_delete":
      findPlan(args.id).deletedAt = now();
      return r(undefined);
    case "planner_restore": {
      const p = plans.find((x) => x.id === args.id && x.deletedAt);
      if (!p) throw new Error("nothing to restore");
      p.deletedAt = null;
      return r(undefined);
    }
    case "idea_list":
      return r(ideas.filter((i) => !i.deletedAt).map(({ deletedAt: _d, ...i }) => i));
    case "idea_create": {
      const i = args.input as Json;
      const title = String(i.title ?? "").trim();
      if (!title) throw new Error("Give your idea a name (200 characters max)");
      const row: IdeaRow = {
        id: newId(), title, description: String(i.description ?? "").trim(), category: String(i.category), budget: String(i.budget),
        duration: String(i.duration), setting: String(i.setting), mood: String(i.mood), deletedAt: null,
      };
      ideas.unshift(row);
      return r(row.id);
    }
    case "idea_delete": {
      const i = ideas.find((x) => x.id === args.id && !x.deletedAt);
      if (!i) throw new Error("this idea doesn't exist anymore");
      i.deletedAt = now();
      return r(undefined);
    }
    case "idea_restore": {
      const i = ideas.find((x) => x.id === args.id && x.deletedAt);
      if (!i) throw new Error("nothing to restore");
      i.deletedAt = null;
      return r(undefined);
    }
    case "question_custom_list":
      return r(customQs.filter((q) => !q.deletedAt).map(({ deletedAt: _d, ...q }) => q));
    case "question_custom_create": {
      const text = String(args.text ?? "").trim();
      if (!text || text.length > 300) throw new Error("Write your question first (300 characters max)");
      if (!CATEGORY_IDS.includes(String(args.category))) throw new Error("pick a category for your question");
      const q: CustomQ = { id: newId(), text, category: String(args.category), createdAt: now(), deletedAt: null };
      customQs.unshift(q);
      const { deletedAt: _d, ...view } = q;
      return r(view);
    }
    case "question_custom_delete": {
      const q = customQs.find((x) => x.id === args.id && !x.deletedAt);
      if (!q) throw new Error("this question doesn't exist anymore");
      q.deletedAt = now();
      return r(undefined);
    }
    case "question_custom_restore": {
      const q = customQs.find((x) => x.id === args.id && x.deletedAt);
      if (!q) throw new Error("there's nothing to restore");
      q.deletedAt = null;
      return r(undefined);
    }
    case "question_answers": {
      const me = profile.me?.id;
      const liveA = answerRows.filter((a) => !a.deletedAt);
      // a partner's answer shows only once I've answered the same question that day
      const visible = liveA.filter(
        (a) => a.userId === me || liveA.some((m) => m.userId === me && m.questionId === a.questionId && m.answeredForDate === a.answeredForDate),
      );
      return r(
        visible
          .map(({ deletedAt: _d, ...a }) => ({ ...a, mine: a.userId === me }))
          .sort((a, b) => b.answeredForDate.localeCompare(a.answeredForDate) || b.createdAt.localeCompare(a.createdAt)),
      );
    }
    case "question_answer_save": {
      const me = profile.me?.id ?? "preview-me";
      const questionId = String(args.questionId ?? "").trim();
      const body = String(args.body ?? "").trim();
      const date = String(args.date ?? "");
      if (!questionId) throw new Error("pick a question to answer");
      if (!body || body.length > 5000) throw new Error("Write a few words first (5000 characters max)");
      if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) throw new Error("date must be YYYY-MM-DD");
      let a = answerRows.find((x) => !x.deletedAt && x.userId === me && x.questionId === questionId && x.answeredForDate === date);
      if (a) Object.assign(a, { body, updatedAt: now() });
      else {
        a = { id: newId(), questionId, userId: me, body, answeredForDate: date, createdAt: now(), updatedAt: now(), deletedAt: null };
        answerRows.push(a);
      }
      const { deletedAt: _d, ...view } = a;
      return r({ ...view, mine: true });
    }
    case "question_answer_delete": {
      const a = answerRows.find((x) => x.id === args.id && !x.deletedAt && x.userId === (profile.me?.id ?? "preview-me"));
      if (!a) throw new Error("you can only remove your own answers");
      a.deletedAt = now();
      return r(undefined);
    }
    case "question_answer_restore": {
      const a = answerRows.find((x) => x.id === args.id && x.deletedAt);
      if (!a) throw new Error("there's nothing to restore");
      if (answerRows.some((x) => !x.deletedAt && x.userId === a.userId && x.questionId === a.questionId && x.answeredForDate === a.answeredForDate))
        throw new Error("you've already answered this question again");
      a.deletedAt = null;
      return r(undefined);
    }
    case "question_daily_smart":
    case "question_daily": {
      const date = String(args.date);
      const qid = String(cmd === "question_daily_smart" ? args.fallbackId : args.questionId);
      if (!dailyQs.has(date)) dailyQs.set(date, qid); // first writer wins
      return r({ id: date, questionId: dailyQs.get(date)! });
    }
    case "mood_list":
      return r([...moodRows.values()].sort((a, b) => b.date.localeCompare(a.date)).slice(0, Number(args.limit ?? 60)));
    case "mood_save": {
      if (!MOOD_IDS.includes(String(args.mood))) throw new Error("pick how you're feeling");
      const m = { date: String(args.date), mood: String(args.mood), note: String(args.note ?? "").trim() };
      moodRows.set(m.date, m);
      return r(m);
    }
    case "mood_delete":
      moodRows.delete(String(args.date));
      return r(undefined);
    case "letter_list":
      return r(
        letterRows
          .filter((l) => !l.deletedAt && (l.senderId === "preview-me" || l.status === "sent"))
          .sort((a, b) => (b.sentAt ?? b.updatedAt).localeCompare(a.sentAt ?? a.updatedAt))
          .map(letterSummary),
      );
    case "letter_get": {
      const l = findLetter(args.id);
      if (l.senderId !== "preview-me" && l.status !== "sent") throw new Error("this letter doesn't exist anymore");
      const hidden = l.senderId !== "preview-me" && l.kind === "capsule" && !l.unlockedAt;
      return r({ ...letterSummary(l), bodyJson: hidden ? "" : l.bodyJson, media: [] });
    }
    case "letter_save_draft": {
      const f = letterFields(args.input as Json);
      if (args.id) {
        const l = findLetter(args.id);
        if (l.status !== "draft" || l.senderId !== "preview-me") throw new Error("this draft isn't available to edit (sent letters can't change)");
        Object.assign(l, f, { updatedAt: now() });
        return r(l.id);
      }
      const l: LetterRow = { ...f, id: newId(), senderId: "preview-me", recipientId: null, status: "draft", sentAt: null, unlockedAt: null, createdAt: now(), updatedAt: now(), deletedAt: null };
      letterRows.push(l);
      return r(l.id);
    }
    case "letter_send": {
      const l = findLetter(args.id);
      if (l.status !== "draft" || l.senderId !== "preview-me") throw new Error("only your own drafts can be sent");
      if (!l.title) throw new Error("Give it a title before sending");
      if (!excerpt(l.bodyJson).trim()) throw new Error("Write a few words first - the page is still empty");
      if (l.kind === "capsule" && !(l.unlockAt && new Date(l.unlockAt).getTime() > Date.now()))
        throw new Error("Pick a moment in the future to open the capsule");
      Object.assign(l, { status: "sent", sentAt: now(), recipientId: profile.partner ? "preview-partner" : null, updatedAt: now() });
      return r(undefined);
    }
    case "letter_delete":
      findLetter(args.id).deletedAt = now();
      return r(undefined);
    case "letter_restore":
      findLetter(args.id, true).deletedAt = null;
      return r(undefined);
    case "letter_unlock": {
      const l = findLetter(args.id);
      if (l.sealed && !l.unlockedAt) {
        if (!letterMoment(l)) throw new Error("Not quite time yet - this capsule is still sealed.");
        Object.assign(l, { title: l.sealed.title, bodyJson: l.sealed.bodyJson, unlockedAt: now() });
      }
      return r({ ...letterSummary(l), bodyJson: l.senderId !== "preview-me" && l.kind === "capsule" && !l.unlockedAt ? "" : l.bodyJson, media: [] });
    }
    case "media_capabilities":
      return r({ converter: false });
    case "media_import":
      throw new Error("Adding photos only works in the desktop app.");
    case "sync_status":
      return r({ state: "off", configured: false, signedIn: false, paired: false, email: null, projectUrl: null, lastSyncAt: null, lastError: null,
        counts: { pendingOps: 0, awaitingDelivery: 0, uploadsPending: 0, downloadsPending: 0, failedUpdates: 0, openConflicts: 0 },
        devices: [], thisDevice: "preview", relayBytes: null, directEnabled: true, directPeer: false });
    case "cupid_status":
      return r({ enabled: false, hasKey: false, privacyAck: false, model: "gemini-flash-latest", dailyQuestion: true, dateIdeas: true, shareBucketTitles: false });
    case "sync_conflicts":
      return r([]);
    case "open_data_dir":
      return r(undefined);
    default:
      if (cmd.startsWith("sync_")) throw new Error("Partner sync works in the desktop app.");
      throw new Error(`preview backend: unknown command ${cmd}`);
  }
}
