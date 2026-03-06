import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog";

interface KeyboardShortcutsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

function Kbd({ children, wide }: { children: React.ReactNode; wide?: boolean }) {
  return (
    <kbd
      className={`inline-flex h-[26px] items-center justify-center rounded-[5px] border border-border bg-gradient-to-b from-muted/80 to-muted px-1.5 font-mono text-[11px] font-semibold tracking-wide text-foreground/80 shadow-[0_1px_0_1px_var(--border),0_2px_3px_-1px_rgba(0,0,0,0.1)] ${wide ? "min-w-[52px]" : "min-w-[26px]"}`}
    >
      {children}
    </kbd>
  );
}

function Sep({ children }: { children: string }) {
  return (
    <span className="px-0.5 text-[10px] font-medium text-muted-foreground/40">{children}</span>
  );
}

function ShortcutRow({ keys, action }: { keys: React.ReactNode; action: string }) {
  return (
    <div className="group flex items-center justify-between py-[7px] transition-colors">
      <span className="text-[13px] text-muted-foreground group-hover:text-foreground transition-colors">{action}</span>
      <div className="flex items-center gap-0.5">{keys}</div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="space-y-0.5">
      <h3
        className="mb-1 text-[11px] font-bold uppercase tracking-[0.12em] text-primary/70"
        style={{ fontFamily: "'Syne', sans-serif" }}
      >
        {title}
      </h3>
      <div className="divide-y divide-border/50">{children}</div>
    </div>
  );
}

export function KeyboardShortcutsDialog({ open, onOpenChange }: KeyboardShortcutsDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="gap-0 overflow-hidden p-0 sm:max-w-[460px]">
        <DialogHeader className="border-b px-5 py-4">
          <DialogTitle className="text-base">Keyboard Shortcuts</DialogTitle>
          <DialogDescription className="text-xs text-muted-foreground/70">
            Navigate your inbox without leaving the keyboard
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-5 px-5 py-4">
          <Section title="Navigation">
            <ShortcutRow
              keys={<><Kbd>j</Kbd><Sep>/</Sep><Kbd>↓</Kbd></>}
              action="Move down"
            />
            <ShortcutRow
              keys={<><Kbd>k</Kbd><Sep>/</Sep><Kbd>↑</Kbd></>}
              action="Move up"
            />
            <ShortcutRow
              keys={<><Kbd wide>Enter</Kbd><Sep>/</Sep><Kbd>o</Kbd></>}
              action="Open item"
            />
            <ShortcutRow
              keys={<><Kbd>⌘</Kbd><Sep>+</Sep><Kbd>1</Kbd><Sep>–</Sep><Kbd>5</Kbd></>}
              action="Switch tab (All / Notes / Issues / PRs / Discussions)"
            />
          </Section>

          <Section title="Selection">
            <ShortcutRow keys={<Kbd>x</Kbd>} action="Toggle selection" />
            <ShortcutRow
              keys={<><Kbd>⇧</Kbd><Sep>+</Sep><Kbd>j</Kbd><Sep>/</Sep><Kbd>↓</Kbd></>}
              action="Select & move down"
            />
            <ShortcutRow
              keys={<><Kbd>⇧</Kbd><Sep>+</Sep><Kbd>k</Kbd><Sep>/</Sep><Kbd>↑</Kbd></>}
              action="Select & move up"
            />
            <ShortcutRow
              keys={<><Kbd>⌘</Kbd><Sep>+</Sep><Kbd>a</Kbd></>}
              action="Select all"
            />
            <ShortcutRow
              keys={<><Kbd>⇧</Kbd><Sep>+</Sep><span className="text-[11px] font-medium text-muted-foreground/60">Click</span></>}
              action="Range select"
            />
          </Section>

          <Section title="Actions">
            <ShortcutRow keys={<Kbd>e</Kbd>} action="Dismiss selected / focused" />
            <ShortcutRow keys={<Kbd>?</Kbd>} action="Show this help" />
          </Section>
        </div>

        <div className="border-t bg-muted/30 px-5 py-2.5">
          <p className="text-center text-[11px] text-muted-foreground/50">
            Press <Kbd>Esc</Kbd> to close
          </p>
        </div>
      </DialogContent>
    </Dialog>
  );
}
