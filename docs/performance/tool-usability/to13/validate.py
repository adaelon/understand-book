"""Validate generated published examples against their complete input schemas."""
import json
from pathlib import Path

from jsonschema import Draft7Validator

ROOT = Path(__file__).resolve().parents[4]
reference = json.loads((ROOT / 'docs/tool-contracts.json').read_text(encoding='utf-8'))
counts = {}
for surface in ('resident', 'local_mcp', 'remote_mcp', 'executor'):
    examples = 0
    outputs = 0
    for tool in reference[surface]:
        schema = tool['inputSchema']
        Draft7Validator.check_schema(schema)
        validator = Draft7Validator(schema)
        for example in schema.get('examples', []):
            validator.validate(example)
            examples += 1
        if tool.get('outputSchema'):
            Draft7Validator.check_schema(tool['outputSchema'])
            outputs += 1
    counts[surface] = {'tools': len(reference[surface]), 'input_examples': examples, 'output_schemas': outputs}

states = reference['tutor_states']
state_examples = 0
for name, state in states.items():
    if not state['spec']:
        continue
    schema = state['spec']['parameters']
    Draft7Validator.check_schema(schema)
    for example in schema['examples']:
        Draft7Validator(schema).validate(example)
        state_examples += 1
counts['tutor_states'] = {
    'visible': sum(bool(state['spec']) for state in states.values()),
    'unavailable': sum(state['spec'] is None for state in states.values()),
    'input_examples': state_examples,
}
counts['model_sampling'] = False
print(json.dumps(counts, ensure_ascii=False, indent=2))
Path(__file__).with_name('validation.json').write_text(json.dumps(counts, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
