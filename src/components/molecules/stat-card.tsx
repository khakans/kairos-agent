import type { LucideIcon } from "lucide-react";
export function StatCard({
  label,
  value,
  detail,
  icon: Icon,
  accent = false,
}: {
  label: string;
  value: string;
  detail: string;
  icon: LucideIcon;
  accent?: boolean;
}) {
  return (
    <div className={`stat-card ${accent ? "stat-accent" : ""}`}>
      <div className="stat-label">
        <span>{label}</span>
        <Icon size={16} />
      </div>
      <div className="stat-value">{value}</div>
      <p>{detail}</p>
    </div>
  );
}
