import { isDisplayFormulaSource } from './md';

type FlowSource = { kind: string; text: string };
export function isReaderFlowSegment(source: FlowSource, heading = /^\s*#{1,6}\s/.test(source.text)): boolean {
  return (source.kind === 'paragraph' && !heading) || (source.kind === 'formula' && !isDisplayFormulaSource(source.text));
}
export function shouldJoinReaderFlow(previous: FlowSource, next: FlowSource): boolean {
  return previous.kind === 'formula' || next.kind === 'formula';
}
