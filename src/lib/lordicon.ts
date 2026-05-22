import type { Player } from "@lordicon/web";

export type LordIconElement = HTMLElement & {
  playerInstance?: Player;
  readyPromise?: Promise<void>;
};

export async function playLordIconOnce(
  el: LordIconElement | null,
  state?: string
) {
  if (!el) return;
  await el.readyPromise;
  const player = el.playerInstance;
  if (!player) return;
  if (state) {
    player.state = state;
  }
  player.loop = false;
  player.playFromStart();
}

/** Play reveal once, then switch to loopState and loop until cleanup(). */
export function playRevealThenLoop(
  el: LordIconElement | null,
  revealState: string,
  loopState: string
): () => void {
  let disposed = false;
  let removeComplete: (() => void) | undefined;

  const cleanup = () => {
    disposed = true;
    removeComplete?.();
    removeComplete = undefined;
    const player = el?.playerInstance;
    if (player) {
      player.loop = false;
      player.stop();
    }
  };

  void (async () => {
    if (!el) return;
    await el.readyPromise;
    if (disposed) return;
    const player = el.playerInstance;
    if (!player) return;

    player.loop = false;
    player.state = revealState;
    player.playFromStart();

    removeComplete = player.addEventListener("complete", () => {
      if (disposed) return;
      removeComplete?.();
      removeComplete = undefined;
      player.loop = true;
      player.state = loopState;
      player.playFromStart();
    });
  })();

  return cleanup;
}

export function resetLordIcon(el: LordIconElement | null, defaultState?: string) {
  if (!el?.playerInstance) return;
  const player = el.playerInstance;
  player.loop = false;
  player.stop();
  if (defaultState) {
    player.state = defaultState;
  }
  player.seekToStart();
}
