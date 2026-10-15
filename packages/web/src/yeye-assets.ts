import idle from './assets/yeye/yeye-idle.webp';
import reading from './assets/yeye/yeye-reading-cream.webp';
import avatar from './assets/yeye/yeye-avatar.webp';
import idleAnimation from './assets/yeye/idle.json?url&no-inline';
import readingAnimation from './assets/yeye/reading.json?url&no-inline';

// Production derivatives and their sources are recorded in assets/brand/yeye/production-v1.
export const yeyeAssets = { idle, reading, avatar } as const;
export type YeyeVariant = keyof typeof yeyeAssets;

export type YeyeAnimation = 'idle' | 'reading';
const animationUrls = { idle: idleAnimation, reading: readingAnimation };

// Only the selected animation is fetched. Keep the original 1254px coordinate
// system; SVG scales the same-canvas production WebP into that image box.
export async function loadYeyeAnimation(variant: YeyeAnimation, signal: AbortSignal) {
  const response = await fetch(animationUrls[variant], { signal });
  if (!response.ok) throw new Error(`Yeye animation: ${response.status}`);
  const data = await response.json();
  for (const asset of data.assets) {
    if (asset.id === 'approved-art') Object.assign(asset, { u: '', p: yeyeAssets[variant], e: 1 });
  }
  return data;
}
