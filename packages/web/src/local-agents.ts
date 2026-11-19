import { ref } from 'vue';
import { localRequest } from './reader-host';

export interface AgentCommand {
  id: string;
  name: string;
  command: string;
  args: string[];
}
export interface LocalAgent extends AgentCommand {
  configured: boolean;
  available: boolean;
  managed: boolean;
  installable: boolean;
  login: string | null;
}
interface AgentSettings {
  agents: LocalAgent[];
  selected_agent_id?: string | null;
  probe?: { agent_info?: { name?: string; version?: string } };
}
export type AgentAction = {action: 'save'; agent: AgentCommand}
  | {action: 'select'; id: string | null}
  | {action: 'probe' | 'install' | 'uninstall' | 'disconnect' | 'connect' | 'update'; id: string};

export function useLocalAgents() {
  const agents = ref<LocalAgent[]>([]);
  const selected = ref('');
  const busy = ref(false);
  const loaded = ref(false);
  const error = ref('');
  async function request(action?: AgentAction) {
    if (busy.value) return;
    busy.value = true;
    error.value = '';
    try {
      const settings = await localRequest<AgentSettings>(action ? 'POST' : 'GET', '/local-agent/settings', action);
      agents.value = settings.agents;
      selected.value = settings.selected_agent_id ?? '';
      loaded.value = true;
      return settings;
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy.value = false;
    }
  }
  return { agents, selected, busy, loaded, error, request };
}
export type LocalAgentController = ReturnType<typeof useLocalAgents>;
