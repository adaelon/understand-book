from pathlib import Path
import shutil, json
root = Path.cwd()
base = root/'tmp/cloud-release-20261021/frontend'
dest = root/'tmp/local-library-20261021/frontend'
for name in ['packages/web', 'packages/core/src']:
    shutil.copytree(base/name, dest/name, dirs_exist_ok=True, ignore=shutil.ignore_patterns('node_modules','dist','dist-multi','target'))
for name in ['package.json','tsconfig.json']:
    shutil.copyfile(base/name, dest/name)
files = ['local-materials.ts','local-epub.ts','components/LocalMaterials.vue','components/LocalMaterialReader.vue','components/LocalPdfReader.vue','components/PersonalMaterials.vue']
for name in files:
    shutil.copyfile(root/'packages/web/src'/name, dest/'packages/web/src'/name)
p = dest/'packages/web/src/NetworkApp.vue'
text = p.read_text(encoding='utf-8')
assert "import PersonalPreparations from './components/PersonalPreparations.vue';" in text
text = text.replace("import PersonalPreparations from './components/PersonalPreparations.vue';", "import PersonalMaterials from './components/PersonalMaterials.vue';")
text = text.replace('<PersonalPreparations :key="selectedLearning.learning_workspace_id"', '<PersonalMaterials :key="`${network.identity.user_id}:${selectedLearning.learning_workspace_id}`"')
p.write_text(text, encoding='utf-8')
p = dest/'packages/web/src/personal-preparations.ts'
text = p.read_text(encoding='utf-8')
text = text.replace('const endpoint=', 'export interface PreparationSource { name: string; size: number; slice(start: number, end: number): { arrayBuffer(): Promise<ArrayBuffer> }; }\nconst endpoint=')
assert 'file:File' in text
p.write_text(text.replace('file:File', 'file:PreparationSource'), encoding='utf-8')
p = dest/'packages/web/src/components/PersonalPreparations.vue'
text = p.read_text(encoding='utf-8').replace('<h3>加入自己的资料</h3><p>把书籍、笔记或视频放进这个工作区，接着一起学。</p>', '<h3>云端资料与准备</h3><p>上传到服务器，在网站和其他设备接着学习。</p>').replace('class="private-label">个人资料', 'class="private-label">云端副本').replace('<strong>私人空间</strong>', '<strong>私人云端空间</strong>')
p.write_text(text, encoding='utf-8')
manifest = {'base': str(base), 'changed': files + ['NetworkApp.vue','personal-preparations.ts','components/PersonalPreparations.vue'], 'server_changes': False, 'database_changes': False}
(dest.parent/'source.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps(manifest, ensure_ascii=False))
