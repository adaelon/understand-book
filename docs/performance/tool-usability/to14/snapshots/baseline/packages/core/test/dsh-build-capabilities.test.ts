import { expect, it } from 'vitest';
import { runAutomaticBuildDriverCommand } from '../../../skills/build/automatic-build-driver';
import { DSH_BUILD_CONTROL_CONTRACT_V1 } from '../src/dsh-build-executor-contract';

it('negotiates DSH control versions before requiring a workspace or creating an invocation', () => {
  expect(runAutomaticBuildDriverCommand({ version: 'dsh_build_capabilities.v1' })).toEqual(DSH_BUILD_CONTROL_CONTRACT_V1);
  expect(() => runAutomaticBuildDriverCommand({ version: 'dsh_build_capabilities.v1', target_input: 'unexpected' })).toThrow();
});
