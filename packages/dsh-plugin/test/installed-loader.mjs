// Run with the installed Electron executable in ELECTRON_RUN_AS_NODE mode.
// Every DSH dependency resolves through the shipped package tree, including Cordis.
import { registerHooks } from 'node:module';
import { pathToFileURL } from 'node:url';
import path from 'node:path';

const root = process.env.UNDERSTAND_BOOK_DSH_INSTALLED_ROOT;
if (!root) throw new Error('UNDERSTAND_BOOK_DSH_INSTALLED_ROOT is required');
const parentURL = pathToFileURL(path.join(root, 'package.json')).href;
registerHooks({
  resolve(specifier, context, next) {
    // Exercise a rebuilt UI artifact against the installed host without altering app.asar.
    const sidebarPatch = process.env.UNDERSTAND_BOOK_DSH_SIDEBAR_PATCH;
    if (sidebarPatch && specifier === '@deepseek-ai/dsh-client-ui-sidebar-right') {
      return next(pathToFileURL(path.join(sidebarPatch, 'lib/index.js')).href, context);
    }
    if (specifier.startsWith('@deepseek-ai/')) {
      return next(specifier, { ...context, parentURL });
    }
    return next(specifier, context);
  },
});
