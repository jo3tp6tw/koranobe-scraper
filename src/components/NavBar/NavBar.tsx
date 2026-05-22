import { useCallback, useRef } from "react";
import type { AppPage } from "../../types/navigation";
import { APP_PAGES } from "../../types/navigation";
import { playLordIconOnce, type LordIconElement } from "../../lib/lordicon";
import "./NavBar.css";

const NAV_ICON_COLORS = "primary:#52525b,secondary:#71717a";

interface NavBarProps {
  current: AppPage;
  onChange: (page: AppPage) => void;
}

interface NavTabProps {
  id: AppPage;
  label: string;
  iconSrc: string;
  iconState: string;
  active: boolean;
  onSelect: (page: AppPage) => void;
}

function NavTab({
  id,
  label,
  iconSrc,
  iconState,
  active,
  onSelect,
}: NavTabProps) {
  const iconRef = useRef<LordIconElement>(null);

  const handleMouseEnter = useCallback(() => {
    void playLordIconOnce(iconRef.current, iconState);
  }, [iconState]);

  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      className={`nav-bar__tab${active ? " nav-bar__tab--active" : ""}`}
      onClick={() => onSelect(id)}
      onMouseEnter={handleMouseEnter}
    >
      <lord-icon
        ref={iconRef}
        className="nav-bar__tab-icon"
        src={iconSrc}
        state={iconState}
        colors={NAV_ICON_COLORS}
      />
      <span className="nav-bar__tab-label">{label}</span>
    </button>
  );
}

export function NavBar({ current, onChange }: NavBarProps) {
  return (
    <nav className="nav-bar">
      <h1 className="nav-bar__brand">小說下載器</h1>
      <div className="nav-bar__tabs" role="tablist">
        {APP_PAGES.map(({ id, label, iconSrc, iconState }) => (
          <NavTab
            key={id}
            id={id}
            label={label}
            iconSrc={iconSrc}
            iconState={iconState}
            active={current === id}
            onSelect={onChange}
          />
        ))}
      </div>
    </nav>
  );
}
