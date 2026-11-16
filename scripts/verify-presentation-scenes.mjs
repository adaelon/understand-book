// Exercise the authored scene's actual graphics, then capture, restore and export through Reader.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import {
  fileURLToPath
} from 'node:url';
import {
  createRequire
} from 'node:module';
const root = fileURLToPath(new URL('../', import.meta.url)),
  web = createRequire(path.join(root, 'packages/web/package.json'));
const {
  chromium
} = web('@playwright/test'), {
  build
} = createRequire(web.resolve('vite/package.json'))('esbuild');
const methods = JSON.parse(fs.readFileSync(path.join(root, 'skills/presentation/methods/catalog.json'), 'utf8'))
  .methods.filter(m => m.scenes && (!process.argv[2] || process.argv.slice(2).includes(m.id)));
const out = path.resolve(process.env.SCENES_OUTPUT ?? path.join(root, 'tmp/presentation-scenes-verification'));
fs.mkdirSync(out, {
  recursive: true
});
const app = await build({
  entryPoints: [path.join(root, 'packages/web/src/presentation-document.ts')],
  bundle: true,
  format: 'iife',
  globalName: 'ProductionDocument',
  write: false,
  plugins: [{
    name: 'raw',
    setup(b) {
      b.onResolve({
        filter: /\?raw$/
      }, a => ({
        path: path.resolve(a.resolveDir, a.path.slice(0, -4)),
        namespace: 'raw'
      }));
      b.onLoad({
        filter: /.*/,
        namespace: 'raw'
      }, a => ({
        contents: fs.readFileSync(a.path, 'utf8'),
        loader: 'text'
      }))
    }
  }]
});
const threePath = 'libraries/three-0.180.0.min.js',
  three = fs.readFileSync(path.join(root, 'assets/presentation/three-0.180.0/three.min.js'), 'utf8');
const observeRenderer =
  ';THREE.Scene.prototype.onBeforeRender=function(renderer,scene,camera){window.observedScene={scene,camera}}';
const browser = await chromium.launch({
  channel: process.env.METHOD_LIBRARY_BROWSER ?? 'msedge',
  headless: true,
  args: ['--enable-unsafe-swiftshader']
});
const results = [];
let n = 0;
async function mount(ctx, method, scene, state = null, noGL = false) {
  const page = await ctx.newPage(),
    errors = [];
  page.on('pageerror', e => errors.push(e.message));
  page.on('request', r => {
    if (/^https?:/.test(r.url())) errors.push(r.url())
  });
  if (noGL) await page.addInitScript(() => {
    const original = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function(type, ...a) {
      return type.startsWith('webgl') ? null : original.call(this, type, ...a)
    }
  });
  await page.setContent(
    '<body style="margin:0"><iframe sandbox="allow-scripts" style="display:block;width:100%;height:100vh;border:0"></iframe></body>'
    );
  await page.addScriptTag({
    content: app.outputFiles[0].text
  });
  const base = path.join(root, method.root),
    meta = JSON.parse(fs.readFileSync(path.join(base, scene.metadata), 'utf8'));
  const view = {
    ...meta,
    entrypoint: 'index.html',
    sources: [],
    restored_state: state,
    content_files: {
      'index.html': fs.readFileSync(path.join(base, scene.file), 'utf8'),
      ...(meta.libraries.includes('three') ? {
        [threePath]: three + observeRenderer
      } : {})
    }
  };
  await page.evaluate(v => {
    window.replies = [];
    addEventListener('message', e => window.replies.push(e.data));
    document.querySelector('iframe').srcdoc = ProductionDocument.presentationDocument(v, 'scenes')
  }, view);
  await page.waitForFunction(() => replies.some(m => m.kind === 'ready'));
  return {
    page,
    f: page.frames()[1],
    errors
  };
}
async function msg(page, kind) {
  const id = String(++n);
  await page.evaluate(({
    kind,
    id
  }) => document.querySelector('iframe').contentWindow.postMessage({
    channel: 'scenes',
    kind,
    request_id: id
  }, '*'), {
    kind,
    id
  });
  await page.waitForFunction(id => replies.some(m => m.request_id === id), id);
  return page.evaluate(id => replies.find(m => m.request_id === id), id)
}
async function range(f, selector, value) {
  await f.locator(selector).evaluate((el, v) => {
    el.value = String(v);
    el.dispatchEvent(new Event('input', {
      bubbles: true
    }));
    el.dispatchEvent(new Event('change', {
      bubbles: true
    }))
  }, value)
}
async function inspect(m, scene, width, height) {
  const ctx = await browser.newContext({
    viewport: {
      width,
      height
    },
    hasTouch: width !== 960,
    deviceScaleFactor: 1,
    locale: 'zh-CN'
  });
  try {
    const {
      page,
      f,
      errors
    } = await mount(ctx, m, scene);
    const mode = scene.file.split('/')[0],
      spatial = m.needs.includes('three'),
      read = () => f.locator('#readout').innerText();
    let observation;
    if (!spatial) {
      for (const [k, a, b] of [
          ['l1', 7, 5],
          ['l2', 5, 5],
          ['linf', 4, 5]
        ]) {
        await f.locator('#' + k).click();
        assert.match(await read(), new RegExp('甲 ' + a + '[\\s，]+乙 ' + b));
        if (mode === 'prints') {
          const widths = await f.locator('.ink').evaluateAll(ns => ns.map(el => el.getBoundingClientRect()
            .width / el.parentElement.clientWidth));
          assert.ok(Math.abs(widths[0] - a / 7) < .02 && Math.abs(widths[1] - b / 7) < .02);
        }
      }
      if (mode === 'routes') {
        await f.locator('#l1').click();
        for (let i = 0; i < 5; i++) await f.locator('#step').click();
        assert.match(await read(), /5 步：甲未到，乙已到/);
        // Observe actual hat pixels at the two predicted grid positions, not the status text.
        const hats = await f.locator('canvas').evaluate(c => [
          [36 + 3 * 29 + 3, 184 - 2 * 29 + 2 - 10],
          [36 - 3, 184 - 5 * 29 - 1 - 10]
        ].map(([x, y]) => [...c.getContext('2d').getImageData(x, y, 1, 1).data].slice(0, 3)));
        assert.deepEqual(hats, [
          [255, 157, 130],
          [128, 222, 208]
        ]);
      } else if (mode === 'example') {
        await f.locator('#l1').click();
        const ext = await f.locator('#boundary').evaluate(el => {
          const b = el.getBBox();
          return [b.x, b.y, b.width, b.height]
        });
        assert.deepEqual(ext, [-100, -100, 200, 200]);
      }
    } else {
      assert.equal(await f.locator('canvas').evaluate(c => !!c.getContext('webgl2')), true);
      if (mode === 'example') {
        for (const cut of [0, 50, 100]) {
          await range(f, '#explode', cut);
          const positions = await f.evaluate(() => observedScene.scene.children.filter(x => x.isGroup).map(x => x
            .position.z));
          assert.equal(positions.length, 5);
          assert.ok(positions.every((z, i) => i === 0 || positions[i - 1] > z));
          if (cut === 100) assert.ok(positions[0] - positions[1] > 2 && positions[3] - positions[4] > 2,
            'separate groups during inspection');
        }
      } else {
        await range(f, '#slice', .5);
        for (const [k, x] of [
            ['l1', .5],
            ['l2', Math.sqrt(.75)],
            ['linf', 1]
          ]) {
          await f.locator('#' + k).click();
          assert.ok((await read()).includes(x.toFixed(3)));
          const geometry = await f.evaluate(() => {
            const objects = observedScene.scene.children,
              cut = objects.find(o => o.isLineLoop),
              plane = objects.find(o => o.geometry?.type === 'PlaneGeometry'),
              p = cut.geometry.attributes.position;
            return {
              plane: plane.position.z,
              normal: new THREE.Vector3(0, 0, 1).applyQuaternion(plane.quaternion).z,
              x: Math.max(...Array.from({
                length: p.count
              }, (_, i) => p.getX(i))),
              heights: [...new Set(Array.from({
                length: p.count
              }, (_, i) => p.getZ(i)))]
            }
          });
          assert.equal(geometry.plane, .5);
          assert.equal(geometry.normal, 1);
          assert.deepEqual(geometry.heights, [.5]);
          assert.ok(Math.abs(geometry.x - x) < 1e-6)
        }
      }
      const before = await f.evaluate(() => observedScene.camera.position.toArray()),
        reading = await read();
      await f.locator('#angle').click();
      assert.notDeepEqual(await f.evaluate(() => observedScene.camera.position.toArray()), before);
      assert.equal(await read(), reading);
      if (mode === 'example') {
        const tags = await f.locator('#labels text').allTextContents();
        assert.deepEqual(tags.sort(), ['镜组', '光圈', '外壳', '感光板', '后盖'].sort());
      }
    }
    observation = await read();
    const snap = (await msg(page, 'snapshot')).state;
    const reopened = await mount(ctx, m, scene, snap);
    assert.deepEqual((await msg(reopened.page, 'snapshot')).state.values.page, snap.values.page);
    assert.equal(await reopened.f.locator('#readout').innerText(), observation);
    const exp = await msg(page, 'export-static');
    assert.equal(exp.kind, 'static-result', exp.message);
    assert.ok(exp.image.svg.includes('readout'));
    if (spatial || mode === 'routes') assert.ok(exp.image.svg.includes('data:image/png;base64,'),
      'live canvas preserved in the export');
    assert.equal(await f.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1), false);
    if (width !== 960) assert.deepEqual(await f.locator('button,input').evaluateAll(ns => ns.filter(el => !el
      .hidden && el.getBoundingClientRect().height > 0 && (el.getBoundingClientRect().width < 43.9 || el
        .getBoundingClientRect().height < 43.9)).map(el => el.id)), []);
    if (width !== 640) {
      await f.evaluate(() => scrollTo(0, 0));
      await page.screenshot({
        path: path.join(out, m.id + '-' + mode + '-' + width + '.png')
      })
    }
    if (spatial && width === 320) {
      const blocked = await mount(ctx, m, scene, snap, true);
      assert.ok(await blocked.f.locator('#fallback').isVisible());
      await blocked.f.locator(mode === 'example' ? '#apart' : '#l2').click();
      assert.notEqual(await blocked.f.locator('#readout').innerText(), observation);
      assert.deepEqual(blocked.errors, []);
    }
    assert.deepEqual(errors, []);
    assert.deepEqual(reopened.errors, []);
    results.push({
      method: m.id,
      file: scene.file,
      width,
      height,
      observation,
      state: snap.values.page
    });
    console.log('PASS', m.id, scene.file, width);
  } finally {
    await ctx.close()
  }
}
try {
  for (const m of methods)
    for (const scene of m.scenes)
      for (const [w, h] of [
          [320, 420],
          [640, 240],
          [960, 720]
        ]) await inspect(m, scene, w, h)
} finally {
  await browser.close();
  fs.writeFileSync(path.join(out, 'results.json'), JSON.stringify(results, null, 2) + '\n')
}
