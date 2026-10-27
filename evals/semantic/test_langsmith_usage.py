import unittest

from langsmith_usage import langsmith_extra, langsmith_usage_metadata


class LangSmithUsageTests(unittest.TestCase):
    def test_deepseek_receipt_is_nested_where_langsmith_reads_it(self):
        # Actual shape and counts from a teaching-review trace that displayed 0.
        usage = {
            'prompt_tokens': 49783,
            'completion_tokens': 26135,
            'total_tokens': 75918,
            'prompt_cache_hit_tokens': 49536,
            'prompt_cache_miss_tokens': 247,
            'prompt_tokens_details': {'cached_tokens': 49536},
            'completion_tokens_details': {'reasoning_tokens': 24205},
        }
        metadata = {'ls_provider': 'deepseek', 'ls_model_name': 'deepseek-v4-flash'}
        extra = langsmith_extra(metadata, usage)
        self.assertEqual(extra, {'metadata': {
            **metadata,
            'usage_metadata': {
                'input_tokens': 49783,
                'output_tokens': 26135,
                'total_tokens': 75918,
                'input_token_details': {'cache_read': 49536},
                'output_token_details': {'reasoning': 24205},
            },
        }})
        self.assertNotIn('usage_metadata', metadata)
        self.assertEqual(usage['prompt_tokens'], 49783)

    def test_openai_compatible_details_and_observed_zero_are_preserved(self):
        self.assertEqual(langsmith_usage_metadata({
            'prompt_tokens': 128,
            'completion_tokens': 0,
            'prompt_tokens_details': {'cached_tokens': 0},
        }), {
            'input_tokens': 128,
            'output_tokens': 0,
            'total_tokens': 128,
            'input_token_details': {'cache_read': 0},
        })

    def test_missing_usage_is_not_reported_as_a_free_call(self):
        for usage in (None, {}, {'prompt_tokens': 128}):
            with self.subTest(usage=usage):
                self.assertEqual(langsmith_extra({'case': 'failed'}, usage),
                                 {'metadata': {'case': 'failed'}})


if __name__ == '__main__':
    unittest.main()
