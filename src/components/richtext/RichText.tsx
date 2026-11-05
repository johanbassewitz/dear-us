import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { Bold, Heading2, Heading3, Italic, List, ListOrdered, Quote, Redo2, Undo2 } from "lucide-react";
import { useEffect } from "react";

// Deliberately small: bold, italic, headings, lists, quotes. Nothing that can embed content.
const extensions = [
  StarterKit.configure({
    heading: { levels: [2, 3] },
    code: false,
    codeBlock: false,
    horizontalRule: false,
    strike: false,
    link: false,
    underline: false,
  }),
];

export function parseDoc(json: string): object | null {
  if (!json) return null;
  try {
    return JSON.parse(json);
  } catch {
    return null;
  }
}

export function RichTextView({ json }: { json: string }) {
  const editor = useEditor({ extensions, editable: false, content: parseDoc(json) ?? "" });
  useEffect(() => {
    editor?.commands.setContent(parseDoc(json) ?? "");
  }, [editor, json]);
  return <EditorContent editor={editor} className="prose-dear" />;
}

interface EditorProps {
  initialJson: string;
  onChange: (json: string) => void;
  placeholder?: string;
  /** Accessible name of the writing area (defaults to the memory wording). */
  label?: string;
}

export function RichTextEditor({ initialJson, onChange, label = "Write about this memory" }: EditorProps) {
  const editor = useEditor({
    extensions,
    content: parseDoc(initialJson) ?? "",
    shouldRerenderOnTransaction: true,
    editorProps: { attributes: { class: "prose-dear min-h-[220px] px-4 py-3 outline-none", "aria-label": label } },
    onUpdate: ({ editor }) => onChange(editor.isEmpty ? "" : JSON.stringify(editor.getJSON())),
  });
  if (!editor) return null;

  const btn = (active: boolean, label: string, run: () => void, icon: React.ReactNode, disabled = false) => (
    <button
      type="button"
      aria-label={label}
      title={label}
      aria-pressed={active}
      disabled={disabled}
      onMouseDown={(e) => e.preventDefault()}
      onClick={run}
      className={`grid h-8 w-8 place-items-center rounded-lg transition-colors ${active ? "bg-accent-soft text-accent-deep" : "text-muted hover:bg-paper-2 hover:text-ink"}`}
    >
      {icon}
    </button>
  );
  const c = () => editor.chain().focus();

  return (
    <div className="overflow-hidden rounded-xl border border-line bg-paper focus-within:border-accent">
      <div className="flex flex-wrap items-center gap-0.5 border-b border-line-soft bg-paper-2/60 px-2 py-1.5">
        {btn(editor.isActive("bold"), "Bold (Ctrl+B)", () => c().toggleBold().run(), <Bold size={15} />)}
        {btn(editor.isActive("italic"), "Italic (Ctrl+I)", () => c().toggleItalic().run(), <Italic size={15} />)}
        <span className="mx-1 h-5 w-px bg-line" />
        {btn(editor.isActive("heading", { level: 2 }), "Heading", () => c().toggleHeading({ level: 2 }).run(), <Heading2 size={16} />)}
        {btn(editor.isActive("heading", { level: 3 }), "Small heading", () => c().toggleHeading({ level: 3 }).run(), <Heading3 size={16} />)}
        <span className="mx-1 h-5 w-px bg-line" />
        {btn(editor.isActive("bulletList"), "Bullet list", () => c().toggleBulletList().run(), <List size={16} />)}
        {btn(editor.isActive("orderedList"), "Numbered list", () => c().toggleOrderedList().run(), <ListOrdered size={16} />)}
        {btn(editor.isActive("blockquote"), "Quote", () => c().toggleBlockquote().run(), <Quote size={15} />)}
        <span className="ml-auto" />
        {btn(false, "Undo", () => c().undo().run(), <Undo2 size={15} />, !editor.can().undo())}
        {btn(false, "Redo", () => c().redo().run(), <Redo2 size={15} />, !editor.can().redo())}
      </div>
      <EditorContent editor={editor} />
    </div>
  );
}
