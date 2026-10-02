import { useEffect, useRef, type ReactNode } from "react";
import { X, ArrowUpRight, Inbox, LoaderCircle } from "lucide-react";
import { agents } from "./data";
export function AgentIcon({ agent }: { agent: string }) {
  const a = agents[agent] ?? { short: "AI", color: "#9aa8ba" };
  return (
    <span
      className="agent-icon"
      style={{ color: a.color, background: `${a.color}19` }}
    >
      {a.short}
    </span>
  );
}
export function Panel({
  title,
  sub,
  action,
  children,
  className = "",
}: {
  title?: string;
  sub?: string;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`panel ${className}`}>
      {title && (
        <div className="panel-head">
          <div>
            <h2>{title}</h2>
            {sub && <p>{sub}</p>}
          </div>
          {action}
        </div>
      )}
      {children}
    </section>
  );
}
export function Metric({
  label,
  value,
  note,
  icon,
  accent,
}: {
  label: string;
  value: string;
  note: ReactNode;
  icon: ReactNode;
  accent?: boolean;
}) {
  return (
    <div className={`metric ${accent ? "accent" : ""}`}>
      <div className="metric-label">
        {label}
        <span>{icon}</span>
      </div>
      <strong>{value}</strong>
      <div className="metric-note">{note}</div>
    </div>
  );
}
export function Progress({
  value,
  color,
}: {
  value: number | null | undefined;
  color?: string;
}) {
  return (
    <div
      className="progress"
      role="meter"
      aria-valuenow={value ?? undefined}
      aria-valuemin={0}
      aria-valuemax={100}
    >
      <i
        style={{
          width: `${Math.max(0, Math.min(100, value ?? 0))}%`,
          background:
            color ??
            ((value ?? 0) > 90
              ? "var(--red)"
              : (value ?? 0) > 75
                ? "var(--amber)"
                : "var(--green)"),
        }}
      />
    </div>
  );
}
export function Empty({
  title,
  detail,
  action,
}: {
  title: string;
  detail: string;
  action?: ReactNode;
}) {
  return (
    <div className="empty">
      <Inbox size={30} />
      <h3>{title}</h3>
      <p>{detail}</p>
      {action}
    </div>
  );
}
export function Status({ value }: { value: string }) {
  const label: Record<string, string> = {
    ready: "已连接",
    error: "查询失败",
    partial: "部分数据",
    unavailable: "不可用",
    unconfigured: "未连接",
  };
  return (
    <span className={`status ${value}`}>
      <i />
      {label[value] ?? value}
    </span>
  );
}
export function Busy({ children }: { children?: ReactNode }) {
  return (
    <span className="inline">
      <LoaderCircle size={15} className="spin" />
      {children}
    </span>
  );
}
export function Modal({
  title,
  children,
  onClose,
  wide = false,
}: {
  title: string;
  children: ReactNode;
  onClose: () => void;
  wide?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const d = ref.current;
    d?.showModal();
    return () => d?.close();
  }, []);
  return (
    <dialog
      ref={ref}
      className={wide ? "wide" : ""}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
      onClick={(e) => {
        if (e.target === ref.current) onClose();
      }}
    >
      <div className="modal-head">
        <h2>{title}</h2>
        <button className="icon-button" aria-label="关闭" onClick={onClose}>
          <X size={20} />
        </button>
      </div>
      <div className="modal-body">{children}</div>
    </dialog>
  );
}
export function Field({
  label,
  children,
  hint,
}: {
  label: string;
  children: ReactNode;
  hint?: string;
}) {
  return (
    <label className="field">
      <span>{label}</span>
      {children}
      {hint && <small>{hint}</small>}
    </label>
  );
}
export function LinkButton({
  children,
  onClick,
}: {
  children: ReactNode;
  onClick: () => void;
}) {
  return (
    <button className="text-button" onClick={onClick}>
      {children}
      <ArrowUpRight size={14} />
    </button>
  );
}
