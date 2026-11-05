import {
  BookHeart,
  CalendarHeart,
  Gift,
  History,
  House,
  ListChecks,
  Mail,
  MessageCircleHeart,
  Settings,
  type LucideIcon,
} from "lucide-react";

export type PageId =
  | "home"
  | "journal"
  | "timeline"
  | "bucket"
  | "planner"
  | "letters"
  | "questions"
  | "special"
  | "settings";

export interface NavItem {
  id: PageId;
  label: string;
  icon: LucideIcon;
}

export const NAV_GROUPS: { name: string; items: NavItem[] }[] = [
  { name: "Our space", items: [{ id: "home", label: "Home", icon: House }] },
  {
    name: "Memories",
    items: [
      { id: "journal", label: "Journal", icon: BookHeart },
      { id: "timeline", label: "Timeline", icon: History },
    ],
  },
  {
    name: "Together",
    items: [
      { id: "bucket", label: "Bucket List", icon: ListChecks },
      { id: "planner", label: "Date Planner", icon: CalendarHeart },
      { id: "letters", label: "Love Letters", icon: Mail },
      { id: "questions", label: "Questions", icon: MessageCircleHeart },
      { id: "special", label: "Special Dates", icon: Gift },
    ],
  },
];

export const SETTINGS_ITEM: NavItem = { id: "settings", label: "Settings & Sync", icon: Settings };

export const ALL_PAGES: NavItem[] = [...NAV_GROUPS.flatMap((g) => g.items), SETTINGS_ITEM];

export function isPageId(v: string): v is PageId {
  return ALL_PAGES.some((p) => p.id === v);
}
