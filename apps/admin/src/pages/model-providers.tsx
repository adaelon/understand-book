import { useEffect, useRef } from 'react';
import { createApp } from 'vue';
import ModelProviderSettings from '@understand-book/web/reader/components/ModelProviderSettings.vue';
import type { ModelSettingsClient, ModelSettingsStatus, ModelTestResult } from '@understand-book/web/reader/desktop-provider';
import { session } from '@/lib/api';
import './model-providers.css';

/** Reuse the Reader editor while retaining the administrator's authenticated transport. */
export function ModelProvidersPage() {
  const element = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!element.current) return;
    const client: ModelSettingsClient = {
      load: () => session.request<ModelSettingsStatus>('/admin/model-settings'),
      save: input => session.request<ModelSettingsStatus>('/admin/model-settings', 'POST', input),
      test: (modelConfigId, input) => session.request<ModelTestResult>('/admin/model-settings/test', 'POST', { model_config_id: modelConfigId, input }),
    };
    const editor = createApp(ModelProviderSettings, { client });
    editor.mount(element.current);
    return () => editor.unmount();
  }, []);
  return <section className="space-y-5">
    <div><h1>模型服务</h1><p className="mt-2 text-sm text-muted-foreground">配置阅读、教学与后台任务使用的模型。保存后用于新任务，正在执行的任务保持原配置。</p></div>
    <div ref={element} className="model-settings-host rounded-xl border bg-card p-4 sm:p-6" />
  </section>;
}
