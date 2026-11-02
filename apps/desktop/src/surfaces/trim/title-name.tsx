import * as React from "react";
import { Pencil } from "lucide-react";

/**
 * SOURCE OF TRUTH KEYWORDS: EditableTitle, inline rename, recording name, title bar name, renameRecording
 * WHAT:  The recording name shown in the window title bar; click (or Enter) to rename inline.
 * WHY:   Naming happens right where the user is looking, without a dialog. Enter or blur
 *        saves, Escape restores the old name; empty names are refused.
 * WHERE: TrimWindow title bar zone.
 */
export function EditableTitle({ value, onCommit }: { value: string; onCommit: (name: string) => void }) {
  const [editing, setEditing] = React.useState(false);
  const [draft, setDraft] = React.useState(value);
  const input = React.useRef<HTMLInputElement>(null);

  React.useEffect(() => {
    if (editing) input.current?.select();
  }, [editing]);

  function start() {
    setDraft(value);
    setEditing(true);
  }
  function commit() {
    const name = draft.trim();
    if (name && name !== value) onCommit(name);
    setEditing(false);
  }

  if (editing) {
    return (
      <input
        ref={input}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
          else if (e.key === "Escape") {
            e.stopPropagation();
            setEditing(false);
          }
        }}
        aria-label="Recording name"
        spellCheck={false}
        className="h-7 w-[300px] rounded-[8px] border border-ring/60 bg-card px-2 text-center text-[13px] font-semibold ring-3 ring-ring/20 outline-none"
      />
    );
  }

  return (
    <button
      type="button"
      onClick={start}
      title="Rename"
      className="group flex h-7 max-w-[340px] items-center gap-1.5 rounded-[8px] px-2 text-[13px] font-semibold outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/50"
    >
      <span className="truncate">{value}</span>
      <Pencil className="size-3 shrink-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100" />
    </button>
  );
}
