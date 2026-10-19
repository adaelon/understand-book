import { expect, it } from 'vitest';
import { videoContentRect, videoRegion } from './video-region';
const frame = { video: { media_id: 'v', media_revision: '1' }, actual_pts_ms: 650000, width: 1920, height: 1080 };
it('maps the same source pixels in wide, narrow, letterboxed and pillarboxed players', () => {
  for (const [width, height] of [[960, 540], [317, 178.3125], [960, 800], [1200, 400]]) {
    const content = videoContentRect({ left: 17, top: 29, width, height }, 1920, 1080);
    const start = { x: content.left + 120 / 1920 * content.width, y: content.top + 480 / 1080 * content.height };
    const end = { x: content.left + 1240 / 1920 * content.width, y: content.top + 630 / 1080 * content.height };
    const expected = { frame, rect_normalized: [120 / 1920, 480 / 1080, 1120 / 1920, 150 / 1080] };
    expect(videoRegion(frame, content, start, end)).toEqual(expected);
    expect(videoRegion(frame, content, end, start)).toEqual(expected);
    expect(videoRegion(frame, content, { x: start.x - .005, y: start.y + .005 }, { x: end.x + .005, y: end.y - .005 })).toEqual(expected);
  }
});
it('rejects starts in black bars and clicks, while clamping drags past the image edge', () => {
  const content = videoContentRect({ left: 0, top: 0, width: 960, height: 800 }, 1920, 1080);
  expect(videoRegion(frame, content, { x: 20, y: 20 }, { x: 100, y: 400 })).toBeNull();
  expect(videoRegion(frame, content, { x: 20, y: 200 }, { x: 20, y: 200 })).toBeNull();
  expect(videoRegion(frame, content, { x: 480, y: 400 }, { x: 2000, y: 2000 })?.rect_normalized).toEqual([.5, .5, .5, .5]);
});
