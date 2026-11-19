<script setup lang="ts">
import { computed, ref } from 'vue';
import { Check, ChevronDown, Plus, RefreshCw } from '@lucide/vue';
import { t } from '../i18n';
import type { AgentCommand, LocalAgentController, LocalAgent } from '../local-agents';

const props = defineProps<{ controller: LocalAgentController }>();
const { agents, selected, busy, error, request } = props.controller;
const edit = ref<AgentCommand | null>(null);
const argsText = ref('');
const feedback = ref('');
const feedbackId = ref('');
const expanded = ref('');
const canSave = computed(() => !!edit.value?.name.trim() && !!edit.value.command.trim());

function openEditor(agent?: LocalAgent) {
  edit.value = agent ? { id: agent.id, name: agent.name, command: agent.command, args: [...agent.args] }
    : { id: 'custom-' + crypto.randomUUID(), name: '', command: '', args: [] };
  argsText.value = edit.value.args.join('\n');
  feedback.value = '';
}
async function save() {
  if (!edit.value || !canSave.value) return;
  const agent = { ...edit.value, name: edit.value.name.trim(), command: edit.value.command.trim(),
    args: argsText.value.split(/\r?\n/).filter(line => line.length > 0) };
  if (await request({ action: 'save', agent })) {
    expanded.value = agent.id;
    edit.value = null;
  }
}
async function act(agent: LocalAgent, action: 'install' | 'update' | 'uninstall' | 'disconnect' | 'probe' | 'connect') {
  feedback.value = '';
  feedbackId.value = agent.id;
  const response = await request({ action, id: agent.id });
  if (response && action === 'probe') {
    const info = response.probe?.agent_info;
    feedback.value = [t('连接成功'), info?.name, info?.version].filter(Boolean).join(' · ');
  }
}
async function testAgent(id: string) {
  expanded.value = id;
  const agent = agents.value.find(item => item.id === id);
  if (agent) await act(agent, 'probe');
  else await request({ action:'probe', id });
}
defineExpose({ testAgent });
function statusOf(agent: LocalAgent) {
  if (agent.managed) return t('应用管理的安装');
  if (agent.configured) return agent.available ? t('已配置') : t('启动命令不可用');
  return agent.available ? t('检测到本机安装') : t('尚未安装');
}
</script>

<template>
  <section class="agents-settings" :aria-label="t('我的 Agent')">
    <header class="agents-heading">
      <div><h4>{{ t('我的 Agent') }}</h4><p>{{ t('使用已登录的本机 Agent 查原文和回答问题。') }}</p></div>
      <button type="button" class="agent-icon" :aria-label="t('重新检测 Agent')" :disabled="busy" @click="feedback = ''; request()"><RefreshCw :size="15" /></button>
    </header>
    <p v-if="error" role="alert" class="agent-error">{{ error }}</p>
    <p v-if="busy" role="status">{{ t('处理中…') }}</p>
    <div class="agents-list">
      <div v-for="agent in agents" :key="agent.id" class="agent-row">
        <button class="agent-row-heading" type="button" :aria-expanded="expanded === agent.id" @click="expanded = expanded === agent.id ? '' : agent.id">
          <span><strong>{{ agent.name }}</strong><small>{{ statusOf(agent) }}</small></span>
          <span class="agent-row-end"><Check v-if="selected === agent.id" :aria-label="t('当前问答 Agent')" :size="16" /><ChevronDown :size="15" :class="{up: expanded === agent.id}"/></span>
        </button>
        <div v-if="expanded === agent.id" class="agent-detail">
          <code>{{ [agent.command, ...agent.args].join(' ') }}</code>
          <p v-if="agent.login">{{ t('登录命令') }} <code>{{ agent.login }}</code></p>
          <div class="agent-actions">
            <button v-if="!agent.configured && agent.available" type="button" :disabled="busy" @click="act(agent, 'connect')">{{ t('连接已有安装') }}</button>
            <button v-if="agent.configured && agent.available && selected !== agent.id" type="button" :disabled="busy" @click="request({ action: 'select', id: agent.id })">{{ t('用于问答') }}</button>
            <button v-if="agent.available" type="button" :disabled="busy" @click="act(agent, 'probe')">{{ t('测试连接') }}</button>
            <button v-if="!agent.available && agent.installable" type="button" :disabled="busy" @click="act(agent, 'install')">{{ t('安装') }}</button>
            <button v-if="agent.managed" type="button" :disabled="busy" @click="act(agent, 'update')">{{ t('更新') }}</button>
            <button type="button" :disabled="busy" @click="openEditor(agent)">{{ t('配置命令') }}</button>
            <button v-if="agent.configured && !agent.managed" type="button" :disabled="busy" @click="act(agent, 'disconnect')">{{ t('断开连接') }}</button>
            <button v-if="agent.managed" type="button" :disabled="busy" @click="act(agent, 'uninstall')">{{ t('卸载') }}</button>
          </div>
          <p v-if="feedback && feedbackId === agent.id" class="agent-success" role="status">{{ feedback }}</p>
        </div>
      </div>
    </div>
    <button v-if="!edit" class="agent-add" type="button" :disabled="busy" @click="openEditor()"><Plus :size="14"/>{{ t('添加自定义 Agent') }}</button>
    <form v-else class="agent-edit" @submit.prevent="save">
      <label>{{ t('显示名称') }}<input v-model="edit.name" required placeholder="OpenCode" /></label>
      <label>{{ t('启动命令') }}<input v-model="edit.command" required spellcheck="false" placeholder="opencode" /></label>
      <label>{{ t('启动参数（每行一个）') }}<textarea v-model="argsText" rows="2" spellcheck="false" placeholder="acp" /></label>
      <div class="agent-actions"><button type="submit" :disabled="busy || !canSave">{{ t('保存配置') }}</button><button type="button" :disabled="busy" @click="edit = null">{{ t('取消') }}</button></div>
    </form>
  </section>
</template>

<style scoped>
.agents-settings { display:grid; gap:12px; color:var(--ink); }
.agents-heading,.agent-row-heading,.agent-actions,.agent-row-end { display:flex; align-items:center; gap:8px; }
.agents-heading,.agent-row-heading { justify-content:space-between; }
.agents-heading h4 { font-size:1rem; margin:0 0 4px; }
.agents-settings p,.agents-settings small,.agent-detail code { margin:0; color:var(--steel); font-size:.8rem; line-height:1.5; }
.agents-settings button { font:inherit; color:inherit; cursor:pointer; }
.agents-settings button:disabled { opacity:.55; cursor:default; }
.agent-icon,.agent-actions button,.agent-add { border:1px solid var(--line); background:transparent; border-radius:7px; }
.agent-icon { display:grid; place-items:center; width:32px; height:32px; padding:0; box-sizing:border-box; flex-shrink:0; }
.agents-list { border:1px solid var(--line); border-radius:10px; overflow:hidden; }
.agent-row + .agent-row { border-top:1px solid var(--line); }
.agent-row-heading { width:100%; text-align:left; border:0; background:transparent; padding:10px 12px; }
.agent-row-heading strong,.agent-row-heading small { display:block; }
.agent-row-end { color:var(--steel); }
.up { transform:rotate(180deg); }
.agent-detail { display:grid; gap:8px; padding:0 12px 12px; }
.agent-detail code { overflow-wrap:anywhere; }
.agent-actions { flex-wrap:wrap; }
.agent-actions button,.agent-add { font-size:.78rem; padding:5px 9px; min-height:30px; }
.agent-add { display:inline-flex; align-items:center; gap:5px; justify-self:start; }
.agent-edit,.agent-edit label { display:grid; gap:5px; }
.agent-edit { gap:9px; }
.agent-edit label { font-size:.82rem; }
.agent-edit input,.agent-edit textarea { font:inherit; color:var(--ink); border:1px solid var(--line); background:transparent; border-radius:6px; padding:7px 9px; min-width:0; }
.agent-edit textarea { resize:vertical; }
.agents-settings .agent-error { color:var(--danger,#a5453b); }
.agent-success { font-weight:600; }
</style>
