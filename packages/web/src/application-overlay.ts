/** Reading overlays stay in the content layer; the personal note window remains independently usable. */
export function applicationOverlayTarget(): HTMLElement {
  return document.getElementById('application-overlays') ?? document.body;
}
