import { useCallback, useRef } from "react";
import {
  playRevealThenLoop,
  resetLordIcon,
  type LordIconElement,
} from "../../lib/lordicon";
import "./FolderField.css";

const FOLDER_ICON = "/lordicon/doodle-color-245-folder-open-in-reveal.json";
const FOLDER_REVEAL_STATE = "in-reveal";
const FOLDER_LOOP_STATE = "hover-pinch";

interface FolderFieldProps {
  value: string;
  onSelect: () => void;
  id?: string;
}

export function FolderField({ value, onSelect, id }: FolderFieldProps) {
  const iconRef = useRef<LordIconElement>(null);
  const animationCleanupRef = useRef<(() => void) | null>(null);

  const handleMouseEnter = useCallback(() => {
    animationCleanupRef.current?.();
    animationCleanupRef.current = playRevealThenLoop(
      iconRef.current,
      FOLDER_REVEAL_STATE,
      FOLDER_LOOP_STATE
    );
  }, []);

  const handleMouseLeave = useCallback(() => {
    animationCleanupRef.current?.();
    animationCleanupRef.current = null;
    resetLordIcon(iconRef.current, FOLDER_REVEAL_STATE);
  }, []);

  return (
    <div className="folder-field">
      <input
        id={id}
        className="folder-field__input"
        type="text"
        value={value}
        readOnly
        placeholder="選擇存放目錄..."
      />
      <button
        type="button"
        className="folder-field__btn"
        title="選擇存放目錄"
        aria-label="選擇存放目錄"
        onClick={onSelect}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
      >
        <span className="folder-field__label">選擇</span>
        <lord-icon
          ref={iconRef}
          className="folder-field__icon"
          src={FOLDER_ICON}
          state={FOLDER_REVEAL_STATE}
        />
      </button>
    </div>
  );
}
