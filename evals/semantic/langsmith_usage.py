"""Normalize Chat Completions usage for standalone Python evaluation traces.

LangSmith reads custom usage from ``extra.metadata.usage_metadata``. Provider
responses stay unchanged in the local evaluation result; these functions only
prepare the trace metadata and perform no I/O.
"""


def langsmith_usage_metadata(usage):
    """Return the observed usage in LangSmith's shape, or None when unavailable."""
    if not usage:
        return None
    input_tokens = usage.get('prompt_tokens')
    output_tokens = usage.get('completion_tokens')
    if input_tokens is None or output_tokens is None:
        return None
    result = {
        'input_tokens': input_tokens,
        'output_tokens': output_tokens,
        'total_tokens': usage.get('total_tokens', input_tokens + output_tokens),
    }
    cached = usage.get('prompt_cache_hit_tokens')
    if cached is None:
        cached = (usage.get('prompt_tokens_details') or {}).get('cached_tokens')
    if cached is not None:
        result['input_token_details'] = {'cache_read': cached}
    reasoning = (usage.get('completion_tokens_details') or {}).get('reasoning_tokens')
    if reasoning is not None:
        result['output_token_details'] = {'reasoning': reasoning}
    return result


def langsmith_extra(metadata, usage):
    """Build ``extra`` without inventing zero usage for a failed/missing receipt."""
    normalized = langsmith_usage_metadata(usage)
    return {'metadata': {
        **metadata,
        **({'usage_metadata': normalized} if normalized is not None else {}),
    }}
