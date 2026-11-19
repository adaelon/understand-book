import path from 'node:path';
import { fileURLToPath } from 'node:url';

const sourceRoot = fileURLToPath(new URL('./src/', import.meta.url));
const cloudModules = /^(?:network-(?:main|client|context|presentation)\.ts|NetworkApp\.vue|account-[^/]+\.ts|cloud-plans\.ts|exam-sprints\.ts|personal-preparations\.ts|components\/(?:Account[^/]+|CloudPlans|ExamSprintPanel|PersonalPreparations|DesktopAuthorization|SharedWorkspaceCatalog|SharedWorkspaceInstance)\.vue)$/;

/** Check the modules Vite actually follows, including imports removed by tree shaking. */
export function readerEdition(multiUser) {
  return {
    name: 'reader-edition',
    transformIndexHtml: {
      order: 'pre',
      handler(html, context) {
        if (context.path !== '/index.html' && context.path !== '/') return html;
        return {
          html: multiUser ? html.replace('src="/src/main.ts"', 'src="/src/network-main.ts"') : html,
          tags: [{ tag: 'meta', attrs: { name: 'understand-book-mode', content: multiUser ? 'multi-user' : 'local' }, injectTo: 'head' }],
        };
      },
    },
    generateBundle() {
      const moduleIds = [...this.getModuleIds()];
      const sourceModules = moduleIds.map(id => path.relative(sourceRoot, id.split('?')[0]).replaceAll('\\', '/'))
        .filter(id => !id.startsWith('../') && !path.isAbsolute(id));
      if (!multiUser) {
        const leaked = [...sourceModules.filter(id => cloudModules.test(id)),
          ...moduleIds.filter(id => /\/(?:apps\/cloud-reader|node_modules\/@understand-book\/cloud-reader)\//.test(id.replaceAll('\\', '/')))];
        if (leaked.length) this.error(`The local Reader imports cloud modules: ${[...new Set(leaked)].join(', ')}. Move the dependency behind reader-host.ts or into the cloud entry.`);
      }
      this.info(`${multiUser ? 'Cloud' : 'Local'} Reader: ${new Set(sourceModules).size} source modules; edition dependency boundary passed.`);
    },
  };
}
