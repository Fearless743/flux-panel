import React, { useEffect } from "react";
import { createPortal } from "react-dom";
import { Loader2, X, SearchX } from "lucide-react";
import { clsx } from "clsx";

/* ---------- Button ---------- */
type BtnVariant = "primary" | "ghost" | "outline" | "danger" | "soft";
type BtnSize = "sm" | "md" | "lg" | "icon";

export function Button({
  variant = "ghost",
  size = "md",
  loading,
  className,
  children,
  ...rest
}: React.ButtonHTMLAttributes<HTMLButtonElement> & { variant?: BtnVariant; size?: BtnSize; loading?: boolean }) {
  return (
    <button
      {...rest}
      disabled={rest.disabled || loading}
      className={clsx(
        "inline-flex items-center justify-center gap-1.5 font-medium rounded-[0.7rem] transition-all select-none whitespace-nowrap",
        "disabled:opacity-45 disabled:pointer-events-none focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400/50",
        size === "sm" && "h-8 px-3 text-tsm",
        size === "md" && "h-10 px-4 text-tbase",
        size === "lg" && "h-11 px-6 text-tmd",
        size === "icon" && "h-9 w-9",
        variant === "primary" && "btn-primary text-white font-semibold",
        variant === "soft" && "bg-blue-400/10 text-blue-200 border border-blue-400/20 hover:bg-blue-400/15",
        variant === "outline" && "border border-white/12 bg-white/[0.03] text-slate-200 hover:bg-white/[0.07] hover:border-white/20",
        variant === "ghost" && "text-slate-300 hover:bg-white/[0.06] hover:text-white",
        variant === "danger" && "bg-red-500/12 text-red-300 border border-red-500/25 hover:bg-red-500/20",
        className
      )}
    >
      {loading && <Loader2 size={15} className="animate-spin" />}
      {children}
    </button>
  );
}

/* ---------- Card ---------- */
export function Card({ className, children, hover }: { className?: string; children: React.ReactNode; hover?: boolean }) {
  return <div className={clsx("card", hover && "card-hover", className)}>{children}</div>;
}
export function CardHead({ title, desc, right }: { title: React.ReactNode; desc?: React.ReactNode; right?: React.ReactNode }) {
  return (
    <div className="flex items-start justify-between gap-3 px-5 pt-4 pb-3">
      <div className="min-w-0">
        <h3 className="text-tlg font-semibold tracking-tight text-white">{title}</h3>
        {desc && <p className="mt-0.5 text-tsm text-slate-400">{desc}</p>}
      </div>
      {right && <div className="flex items-center gap-2 shrink-0">{right}</div>}
    </div>
  );
}

/* ---------- Field ---------- */
export function Field({ label, hint, children, className }: { label?: string; hint?: string; children: React.ReactNode; className?: string }) {
  return (
    <label className={clsx("block", className)}>
      {label && <span className="mb-1.5 block text-tsm font-medium text-slate-300">{label}</span>}
      {children}
      {hint && <span className="mt-1 block text-txs text-slate-500">{hint}</span>}
    </label>
  );
}
export const Input = React.forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>(function Input({ className, ...rest }, ref) {
  return <input ref={ref} {...rest} className={clsx("flux-input", className)} />;
});
export const Textarea = React.forwardRef<HTMLTextAreaElement, React.TextareaHTMLAttributes<HTMLTextAreaElement>>(function Textarea({ className, ...rest }, ref) {
  return <textarea ref={ref} {...rest} className={clsx("flux-textarea", className)} />;
});
export function Select({ className, children, ...rest }: React.SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <div className={clsx("relative", className)}>
      <select {...rest} className="flux-select pr-9">
        {children}
      </select>
      <svg className="pointer-events-none absolute right-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
        <path strokeLinecap="round" strokeLinejoin="round" d="m19 9-7 7-7-7" />
      </svg>
    </div>
  );
}

/* ---------- Badge ---------- */
export function Badge({ tone = "slate", children, className, dot }: { tone?: "slate" | "green" | "red" | "amber" | "cyan" | "violet"; children: React.ReactNode; className?: string; dot?: boolean }) {
  const tones: Record<string, string> = {
    slate: "bg-white/[0.06] text-slate-300 border-white/10",
    green: "bg-emerald-400/10 text-emerald-300 border-emerald-400/25",
    red: "bg-red-400/10 text-red-300 border-red-400/25",
    amber: "bg-amber-400/10 text-amber-300 border-amber-400/25",
    cyan: "bg-blue-400/10 text-blue-300 border-blue-400/25",
    violet: "bg-blue-500/15 text-blue-300 border-blue-500/30",
  };
  const dotColor: Record<string, string> = {
    slate: "bg-slate-400", green: "bg-emerald-400", red: "bg-red-400",
    amber: "bg-amber-400", cyan: "bg-blue-400", violet: "bg-blue-500",
  };
  return (
    <span className={clsx("inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-txs font-medium", tones[tone], className)}>
      {dot && <span className={clsx("h-1.5 w-1.5 rounded-full animate-pulse", dotColor[tone])} />}
      {children}
    </span>
  );
}

/* ---------- Switch ---------- */
export function Switch({ checked, onChange, disabled }: { checked: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={(e) => { e.stopPropagation(); onChange(!checked); }}
      className={clsx(
        "relative h-6 w-11 shrink-0 rounded-full transition-colors duration-200",
        checked ? "bg-blue-600" : "bg-white/10 border border-white/10",
        disabled && "opacity-40 pointer-events-none"
      )}
    >
      <span className={clsx(
        "absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-all duration-200",
        checked ? "left-[1.375rem]" : "left-0.5"
      )} />
    </button>
  );
}

/* ---------- Progress ---------- */
/* 按用量自动变色：>=95% 红、>=80% 琥珀、其余蓝，用于配额预警；tone 可强制覆盖 */
export function Progress({ value, className, tone = "auto" }: { value: number; className?: string; tone?: "auto" | "blue" | "emerald" | "amber" | "red" }) {
  const v = Math.max(0, Math.min(100, value));
  const auto = v >= 95 ? "bg-red-500" : v >= 80 ? "bg-amber-400" : "bg-blue-500";
  const fixed = { blue: "bg-blue-500", emerald: "bg-emerald-400", amber: "bg-amber-400", red: "bg-red-500" };
  return (
    <div className={clsx("h-1.5 w-full overflow-hidden rounded-full bg-white/[0.07]", className)}>
      <div
        className={clsx("h-full rounded-full transition-all duration-500", tone === "auto" ? auto : fixed[tone])}
        style={{ width: `${v}%` }}
      />
    </div>
  );
}

/* ---------- Modal ---------- */
export function Modal({
  open, onClose, title, desc, children, footer, width = "max-w-lg",
}: {
  open: boolean; onClose: () => void; title: React.ReactNode; desc?: React.ReactNode;
  children: React.ReactNode; footer?: React.ReactNode; width?: string;
}) {
  useEffect(() => {
    if (!open) return;
    const fn = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", fn);
    document.body.style.overflow = "hidden";
    return () => { window.removeEventListener("keydown", fn); document.body.style.overflow = ""; };
  }, [open, onClose]);
  if (!open) return null;
  return createPortal(
    <div className="modal-overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={clsx("modal-panel card w-full !rounded-2xl", width)}>
        <div className="flex items-start justify-between gap-3 border-b border-white/[0.07] px-5 py-4">
          <div>
            <h2 className="text-tlg font-semibold tracking-tight text-white">{title}</h2>
            {desc && <p className="mt-0.5 text-tsm text-slate-400">{desc}</p>}
          </div>
          <button onClick={onClose} className="rounded-lg p-1.5 text-slate-400 hover:bg-white/[0.07] hover:text-white transition-colors" aria-label="关闭">
            <X size={17} />
          </button>
        </div>
        <div className="overflow-y-auto px-5 py-4">{children}</div>
        {footer && <div className="flex items-center justify-end gap-2 border-t border-white/[0.07] px-5 py-3.5">{footer}</div>}
      </div>
    </div>,
    document.body
  );
}

/* ---------- Page header ---------- */
export function PageHeader({ title, desc, actions }: { title: string; desc?: string; actions?: React.ReactNode }) {
  return (
    <div className="mb-5 flex flex-wrap items-end justify-between gap-3">
      <div>
        <h1 className="text-t2xl font-bold tracking-tight text-white">{title}</h1>
        {desc && <p className="mt-1 text-tsm text-slate-400">{desc}</p>}
      </div>
      {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
    </div>
  );
}

/* ---------- Empty / Loading ---------- */
export function Empty({ text = "暂无数据", action }: { text?: string; action?: React.ReactNode }) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 py-14 text-center">
      <div className="flex h-14 w-14 items-center justify-center rounded-2xl border border-white/10 bg-white/[0.04] text-slate-500">
        <SearchX size={24} />
      </div>
      <p className="text-sm text-slate-400">{text}</p>
      {action}
    </div>
  );
}
export function Loading({ text = "加载中…" }: { text?: string }) {
  return (
    <div className="flex items-center justify-center gap-2.5 py-16 text-slate-400">
      <Loader2 size={18} className="animate-spin text-blue-300" />
      <span className="text-sm">{text}</span>
    </div>
  );
}

/* ---------- Stat ---------- */
export function Stat({ label, value, sub, icon, accent }: { label: string; value: React.ReactNode; sub?: React.ReactNode; icon?: React.ReactNode; accent?: string }) {
  return (
    <Card hover className="relative overflow-hidden p-5">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="text-txs font-medium text-slate-400">{label}</p>
          <p className="mt-1.5 truncate text-t2xl font-bold tracking-tight text-white">{value}</p>
          {sub && <div className="mt-1.5 text-txs text-slate-500">{sub}</div>}
        </div>
        {icon && <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-white/10 text-slate-300" style={accent ? { background: accent } : undefined}>{icon}</div>}
      </div>
    </Card>
  );
}
