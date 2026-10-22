import { randomUUID } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import type { ExternalBuildCostConnection } from './external-build-cost';
import type { VideoChunk } from './video-transcript';

export interface TranscriptServiceMeter {
  transcribe(chunk: VideoChunk, output: string, execute: () => Promise<void>): Promise<void>;
}
interface ServiceReceipt {
  usage_id: string; status: 'completed' | 'unknown' | 'not_started'; quantity_ms?: number;
  evidence: Record<string, unknown>;
}
interface UsageRecord {
  usage_id: string; logical_usage_id: string; attempt: number; quantity_ms: number;
  state: 'prepared' | 'admitted' | 'started' | 'receipt_pending' | 'recorded';
  receipt?: ServiceReceipt;
  processing_confirmation?: string;
}

/** The persisted receipt is written before sending it. A response-loss retry
 * repeats the same usage identity, never repeats recognition. */
export class TranscriptServiceLedger implements TranscriptServiceMeter {
  constructor(private readonly task: string, private readonly workspace: string,
    private readonly connection: ExternalBuildCostConnection, private readonly fetcher: typeof fetch = fetch,
    private readonly processingConfirmation?: string) {}
  private async request(suffix: string, body?: unknown): Promise<any> {
    const response = await this.fetcher(`${this.connection.base_url.replace(/\/$/u, '')}/api/admin/build-tasks/${encodeURIComponent(this.task)}${suffix}`, {
      method: body === undefined ? 'GET' : 'POST',
      headers: {'Content-Type':'application/json', Cookie:this.connection.cookie, Origin:this.connection.origin, 'X-CSRF-Token':this.connection.csrf_token},
      ...(body === undefined ? {} : {body:JSON.stringify(body)}), signal:AbortSignal.timeout(15000),
    });
    if (!response.ok) {
      const error = await response.json().catch(() => null);
      throw new Error(error?.error_code ?? `service_usage_http_${response.status}`);
    }
    return response.json();
  }
  async transcribe(chunk: VideoChunk, output: string, execute: () => Promise<void>) {
    const file = path.join(this.workspace,'.build','service-usage',`${chunk.id}.json`);
    const save = (record: UsageRecord) => {
      mkdirSync(path.dirname(file),{recursive:true});
      writeFileSync(`${file}.tmp`,JSON.stringify(record));
      renameSync(`${file}.tmp`,file);
    };
    let record: UsageRecord | undefined = existsSync(file) ? JSON.parse(readFileSync(file,'utf8')) : undefined;
    const quantity = Math.ceil(chunk.input_end_ms - chunk.input_start_ms);
    const completion = (): ServiceReceipt => {
      const raw = JSON.parse(readFileSync(output,'utf8'));
      const actual = Math.round(Number(raw.info?.duration) * 1000);
      if (!Number.isSafeInteger(actual) || actual <= 0 || actual > quantity + 1)
        throw new Error('SERVICE_USAGE_DURATION_MISSING: Whisper 未返回有效的实际处理音频时长');
      return {usage_id:record!.usage_id,status:'completed',quantity_ms:Math.min(actual,quantity),
        evidence:{source:'whisper_output',chunk_id:chunk.id,input_start_ms:chunk.input_start_ms,
          input_end_ms:chunk.input_end_ms,decoded_duration_ms:actual}};
    };
    if (record && record.quantity_ms !== quantity) throw new Error('service_usage_input_changed');
    const send = async () => {
      record!.state='receipt_pending'; save(record!);
      await this.request('/service-receipts',record!.receipt);
      record!.state='recorded'; save(record!);
    };
    if (record?.receipt && record.state !== 'recorded') await send();
    if (record?.receipt?.status === 'completed') {
      if (!existsSync(output)) throw new Error('service_usage_completed_output_missing');
      return;
    }
    if (existsSync(output)) {
      // The caller validates a saved Whisper response before reporting completion.
      await execute();
      // Pre-existing local/previously published work is reused without a new charge.
      if (!record) return;
      record.receipt=completion();
      await send(); return;
    }
    if (record?.state === 'started' || record?.receipt?.status === 'unknown') {
      const status = await this.request(`/service-charges/${encodeURIComponent(record.usage_id)}`);
      if (status.charge.state === 'released' || status.charge.state === 'pending'
        && this.processingConfirmation && this.processingConfirmation !== record.processing_confirmation) {
        record = {...record,usage_id:`svc_${randomUUID()}`,attempt:record.attempt+1,state:'prepared',receipt:undefined,
          processing_confirmation:this.processingConfirmation};
      } else {
        record.receipt ??= {usage_id:record.usage_id,status:'unknown',evidence:{source:'whisper_host_recovery',reason:'started_without_complete_output',chunk_id:chunk.id}};
        if (record.state !== 'recorded') await send();
        throw new Error('SERVICE_USAGE_UNRESOLVED: 上次转写用量尚未确认，保留预占额度；请明确继续后重试未完成片段');
      }
    }
    record ??= {usage_id:`svc_${randomUUID()}`,logical_usage_id:`transcript:${randomUUID()}:${chunk.id}`,
      attempt:1,quantity_ms:quantity,state:'prepared',processing_confirmation:this.processingConfirmation};
    save(record);
    if (record.state === 'prepared') {
      const admitted = await this.request('/service-admit',{usage_id:record.usage_id,logical_usage_id:record.logical_usage_id,
        attempt:record.attempt,service_kind:'video_transcription',quantity_ms:quantity,processing_confirmation:record.processing_confirmation});
      if (admitted.admitted !== true || admitted.usage_id !== record.usage_id) throw new Error('service_admission_invalid');
      record.state='admitted';save(record);
    }
    record.state='started';save(record);
    try {
      await execute();
      record.receipt=completion();
    } catch (error) {
      record.receipt={usage_id:record.usage_id,status:'unknown',evidence:{source:'whisper_host',reason:'recognition_interrupted',chunk_id:chunk.id}};
      await send().catch(() => undefined);
      throw error;
    }
    await send();
  }
}
