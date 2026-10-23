"""Optional language-neutral protocol demo tests; production plugins are Rust."""
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest


class PluginProtocolExamples(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        examples = Path(__file__).resolve().parent
        package = examples / "rust-plugin"
        subprocess.run(["cargo", "build", "--release", "--locked", "--manifest-path", str(package / "Cargo.toml")],
                       check=True, capture_output=True, timeout=120)
        cls.binary = package / "target/release" / ("i-plugin-example.exe" if os.name == "nt" else "i-plugin-example")
        cls.python = examples / "word_count.py"

    def invoke(self, argv, event):
        process = subprocess.run(argv, input=json.dumps(dict(event, id=1)) + "\n", text=True, encoding="utf-8",
                                 capture_output=True, timeout=10, check=True)
        reply = json.loads(process.stdout)
        self.assertEqual(reply["id"], 1)
        self.assertNotIn("error", reply)
        return reply["data"]

    def test_rust_and_python_use_identical_tool_contract(self):
        event = {"event": "tool", "workspace": ".", "data": {"name": "rust_word_count", "arguments": {"text": "hello 世界"}}}
        rust = self.invoke([str(self.binary)], event)
        python = self.invoke([sys.executable, str(self.python)], event)
        self.assertEqual(json.loads(rust["output"]), json.loads(python["output"]))
        self.assertEqual(json.loads(rust["output"]), {"words": 2, "characters": 8})

    def test_replaceable_local_compactor_uses_the_same_persistent_protocol(self):
        history = [dict(role=role, content='中文 ' * 500) for role in
                   ('system', 'user', 'assistant', 'tool', 'assistant', 'user', 'assistant', 'user')]
        events = [
            dict(id=1, event='compact_plan', data=dict(history=history, compact=True)),
            dict(id=2, event='before_compact', data=dict(messages=history[1:4])),
            dict(id=3, event='after_compact', data=dict(summary='preserve this')),
        ]
        result = subprocess.run([str(self.binary), '--compact'],
                                input=''.join(json.dumps(event) + '\n' for event in events),
                                text=True, encoding='utf-8', capture_output=True, check=True, timeout=10)
        replies = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual([reply['id'] for reply in replies], [1, 2, 3])
        self.assertTrue(all('data' in reply for reply in replies))
        self.assertEqual(replies[0]['data']['cut'], 4)
        self.assertLessEqual(len(replies[1]['data']['summary'].encode('utf-8')), 2048)
        self.assertIn('中文', replies[1]['data']['summary'])
        self.assertEqual(replies[2]['data']['summary'], 'preserve this')

    def test_rust_hook_and_unknown_events(self):
        event = {"event": "system_prompt", "workspace": ".", "data": {"prompt": "base"}}
        self.assertIn("explicit test results", self.invoke([str(self.binary)], event)["prompt"])
        event["event"] = "future_event"
        self.assertEqual(self.invoke([str(self.binary)], event), event["data"])


if __name__ == "__main__":
    unittest.main()
