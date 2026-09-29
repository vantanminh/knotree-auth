import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import worker_config


class WorkerConfigTests(unittest.TestCase):
    def setUp(self):
        self.config = json.loads(Path(__file__).with_name("worker.example.json").read_text())
        self.env = {"CLOUDFLARE_API_TOKEN": "test", "CLOUDFLARE_ACCOUNT_ID": "a" * 32, "WORKER_CONFIG_JSON": json.dumps(self.config)}

    def test_every_missing_worker_setting_fails(self):
        for key in self.env:
            env = dict(self.env); del env[key]
            with patch.dict(os.environ, env, clear=True), patch("sys.argv", ["worker_config.py", "/tmp/not-written"]), self.assertRaisesRegex(ValueError, key):
                worker_config.main()

    def test_complete_worker_settings_write_only_config(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / "worker.json"
            with patch.dict(os.environ, self.env, clear=True), patch("sys.argv", ["worker_config.py", str(target)]):
                worker_config.main()
            self.assertEqual(json.loads(target.read_text()), self.config)
            self.assertNotIn("CLOUDFLARE_API_TOKEN", target.read_text())

    def test_non_https_origin_is_rejected(self):
        self.config["vars"]["API_ORIGIN"] = "http://attacker.example"
        env = {**self.env, "WORKER_CONFIG_JSON": json.dumps(self.config)}
        with patch.dict(os.environ, env, clear=True), patch("sys.argv", ["worker_config.py", "/tmp/not-written"]), self.assertRaises(ValueError):
            worker_config.main()
