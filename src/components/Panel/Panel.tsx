import type { ReactNode } from "react";
import "./Panel.css";

interface PanelProps {
  title?: string;
  meta?: ReactNode;
  children: ReactNode;
  fill?: boolean;
  panelClassName?: string;
  bodyClassName?: string;
}

export function Panel({
  title,
  meta,
  children,
  fill = false,
  panelClassName = "",
  bodyClassName = "",
}: PanelProps) {
  const classNames = ["panel", fill ? "panel--fill" : "", panelClassName]
    .filter(Boolean)
    .join(" ");
  const bodyClasses = ["panel-body", bodyClassName].filter(Boolean).join(" ");

  return (
    <section className={classNames}>
      {title && (
        <header className="panel-header">
          <h3>{title}</h3>
          {meta}
        </header>
      )}
      <div className={bodyClasses}>{children}</div>
    </section>
  );
}
