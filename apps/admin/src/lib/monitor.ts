export interface Capacity {
  total_bytes: number;
  used_bytes: number;
  available_bytes: number;
  used_percent: number;
}
export interface MonitorSample {
  sampled_at: number;
  service_uptime_seconds: number;
  memory: Capacity | null;
  disk: Capacity | null;
  network: {
    interfaces: string[];
    receive_bytes_per_second: number | null;
    transmit_bytes_per_second: number | null;
  } | null;
  activity: {
    online_users: number;
    active_runs: number;
    queued_runs: number;
    active_run_limit: number;
    today_turns: number;
    today_completed_turns: number;
  };
  issues: string[];
}
export interface MonitorReport {
  current: MonitorSample;
  history: MonitorSample[];
  interval_seconds: number;
  history_seconds: number;
  online_window_seconds: number;
  timezone: string;
  platform: string;
}

export function bytes(value: number): string {
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  const index = Math.min(units.length - 1, Math.max(0, Math.floor(Math.log2(value || 1) / 10)));
  return `${(value / 1024 ** index).toLocaleString('zh-CN', { maximumFractionDigits: index ? 1 : 0 })} ${units[index]}`;
}

export function bandwidth(bytesPerSecond: number | null | undefined): string {
  if (bytesPerSecond == null) return '待采样';
  const bits = bytesPerSecond * 8;
  const unit = bits >= 1e9 ? 'Gbps' : bits >= 1e6 ? 'Mbps' : bits >= 1e3 ? 'Kbps' : 'bps';
  const divisor = unit === 'Gbps' ? 1e9 : unit === 'Mbps' ? 1e6 : unit === 'Kbps' ? 1e3 : 1;
  return `${(bits / divisor).toLocaleString('zh-CN', { maximumFractionDigits: 1 })} ${unit}`;
}

export function uptime(seconds: number): string {
  if (seconds < 60) return `${seconds} 秒`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)} 分钟`;
  if (seconds < 86400) return `${Math.floor(seconds / 3600)} 小时 ${Math.floor(seconds % 3600 / 60)} 分钟`;
  return `${Math.floor(seconds / 86400)} 天 ${Math.floor(seconds % 86400 / 3600)} 小时`;
}

export function monitorTime(seconds: number): string {
  return new Date(seconds * 1000).toLocaleTimeString('zh-CN', { timeZone: 'Asia/Hong_Kong', hour12: false });
}

/** Break the line at unavailable readings and missed intervals instead of drawing invented values. */
export function trendSegments(samples: MonitorSample[], read: (sample: MonitorSample) => number | null | undefined) {
  const groups: { at: number; value: number }[][] = [];
  let current: { at: number; value: number }[] = [];
  for (const sample of samples) {
    const value = read(sample);
    if (value == null || !Number.isFinite(value) || (current.length > 0 && sample.sampled_at - current[current.length - 1].at > 15)) {
      if (current.length) groups.push(current);
      current = [];
    }
    if (value != null && Number.isFinite(value)) current.push({ at: sample.sampled_at, value });
  }
  if (current.length) groups.push(current);
  return groups;
}
