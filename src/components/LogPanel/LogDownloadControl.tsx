import { useCallback, useRef } from "react";
import { playLordIconOnce, type LordIconElement } from "../../lib/lordicon";
import "./LogDownloadControl.css";

const LOADER_ICON =
  "/lordicon/system-solid-715-spinner-horizontal-dashed-circle-loop-transparency.json";
const CROSS_ICON = "/lordicon/system-solid-29-cross-hover-cross-3.json";
const CROSS_STATE = "hover-cross-3";

interface LogDownloadControlProps {
  onStop: () => void;
}

export function LogDownloadControl({ onStop }: LogDownloadControlProps) {
  const crossRef = useRef<LordIconElement>(null);

  const handleMouseEnter = useCallback(() => {
    void playLordIconOnce(crossRef.current, CROSS_STATE);
  }, []);

  return (
    <button
      type="button"
      className="log-download-control"
      title="強制停止下載"
      aria-label="強制停止下載"
      onClick={onStop}
      onMouseEnter={handleMouseEnter}
    >
      <lord-icon
        className="log-download-control__icon log-download-control__loader"
        src={LOADER_ICON}
        trigger="loop"
        state="loop-transparency"
        colors="primary:#ececec,secondary:#ececec"
      />
      <lord-icon
        ref={crossRef}
        className="log-download-control__icon log-download-control__cross"
        src={CROSS_ICON}
        state={CROSS_STATE}
        colors="primary:#ececec,secondary:#ececec"
      />
    </button>
  );
}
