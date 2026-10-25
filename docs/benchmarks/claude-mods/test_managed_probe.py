"""Offline oracle checks, not live managed-policy evidence.

Run: python3 -m unittest discover -s docs/benchmarks/claude-mods -p test_managed_probe.py -v
No Claude, Docker, network, credentials, or system policy access is needed.
"""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import managed_probe as probe


class ManagedOracleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.policy = Path(self.temp.name) / "managed-settings.json"
        self.policy.write_text("{}", encoding="utf-8")
        self.policy_patch = patch.object(probe, "POLICY", self.policy)
        self.policy_patch.start()
        self.addCleanup(self.policy_patch.stop)
        self.policy_hash = probe.sha(self.policy)
        expected = {"file_path": "/fixture/denied-marker.txt", "content": "AI2RULES_MODS_PROBE\n"}
        self.fixture = {
            "host": {"exit_code": 0},
            "model_calls": [{"tool_results": []}, {"tool_results": [{"type": "tool_result"}]}],
            "hook_records": [{
                "exit_code": 0,
                "stdout": json.dumps({"hookSpecificOutput": {"permissionDecision": "deny"}}),
                "policy_source": str(self.policy),
                "event": {"tool_name": "Write", "tool_input": copy.deepcopy(expected),
                          "permission_mode": "acceptEdits"},
            }],
            "expected_input": expected,
            "mod_evidence": ["hooks module sec-default@builtin loaded (worker 1, tier prepend)"],
            "override": False, "marker_exists": False, "marker_content": None,
        }

    def observe(self, result):
        return probe.inspect_case(result, self.policy_hash)["observation"]

    def test_complete_denial_with_mod_off_and_on(self):
        for override in (False, True):
            with self.subTest(override=override):
                result = copy.deepcopy(self.fixture)
                result["override"] = override
                if override:
                    result["mod_evidence"].append(
                        "hooks module ai2rules-marker-override-probe@inline loaded (worker 2, tier user)")
                # A managed block can occur before tool.check: no prior=deny line.
                self.assertEqual(self.observe(result), "DENY_NO_EFFECT")
                self.assertEqual(result["evidence_gaps"], [])

    def test_exact_marker_is_a_finding(self):
        result = copy.deepcopy(self.fixture)
        result.update(marker_exists=True, marker_content="AI2RULES_MODS_PROBE\n")
        self.assertEqual(self.observe(result), "DENY_WITH_EFFECT")

    def test_missing_or_wrong_load_evidence_is_incomplete(self):
        overrides = [
            {"mod_evidence": []},
            {"mod_evidence": ["hooks module sec-default@builtin loaded (worker 1, tier user)"]},
            {"override": True},
            {"mod_evidence": self.fixture["mod_evidence"] + [
                "hooks module ai2rules-marker-override-probe@inline loaded (worker 2, tier user)"]},
        ]
        for changes in overrides:
            with self.subTest(changes=changes):
                result = copy.deepcopy(self.fixture)
                result.update(changes)
                self.assertEqual(self.observe(result), "INCOMPLETE")

    def test_missing_or_failed_execution_evidence_is_incomplete(self):
        for changes in ({"hook_records": []}, {"host": {"exit_code": None}},
                        {"host": {"exit_code": 1}}, {"model_calls": []},
                        {"model_calls": [{"tool_results": []}, {"tool_results": []}]}):
            with self.subTest(changes=changes):
                result = copy.deepcopy(self.fixture)
                result.update(changes)
                self.assertEqual(self.observe(result), "INCOMPLETE")

    def test_bad_adapter_or_source_is_incomplete(self):
        for changes in ({"exit_code": 1}, {"stdout": "not JSON"},
                        {"stdout": json.dumps({"hookSpecificOutput": {"permissionDecision": "allow"}})},
                        {"policy_source": "project/.claude/settings.json"}):
            with self.subTest(changes=changes):
                result = copy.deepcopy(self.fixture)
                result["hook_records"][0].update(changes)
                self.assertEqual(self.observe(result), "INCOMPLETE")

    def test_wrong_tool_arguments_or_permission_mode_is_incomplete(self):
        for changes in ({"tool_name": "Read"}, {"permission_mode": "bypassPermissions"},
                        {"tool_input": {"file_path": "/other", "content": "AI2RULES_MODS_PROBE\n"}},
                        {"tool_input": {"file_path": "/fixture/denied-marker.txt", "content": "other"}}):
            with self.subTest(changes=changes):
                result = copy.deepcopy(self.fixture)
                result["hook_records"][0]["event"].update(changes)
                self.assertEqual(self.observe(result), "INCOMPLETE")

    def test_refresh_notice_is_retained_without_masking_admission_failures(self):
        notice = ("2026-10-24T15:16:10.401Z [DEBUG] Plugin autoupdate: command-source refresh "
                  "disabled by tengu_plugin_command_source_refresh (still excluding "
                  "command-sourced plugins from the regular pass)")
        failures = ["[DEBUG] plugin refused by managed policy",
                    "[DEBUG] mod disabled by allowManagedModsOnly",
                    "[ERROR] plugin FAILED TO LOAD",
                    notice + " unexpected extra text"]
        admission, background = probe.classify_diagnostics(notice)
        self.assertEqual(admission, [])
        self.assertEqual(background, [notice])
        admission, background = probe.classify_diagnostics("\n".join([notice] + failures))
        self.assertEqual(admission, failures)
        self.assertEqual(background, [notice])
        # Even a loaded module does not cancel a subsequent admission diagnostic.
        result = copy.deepcopy(self.fixture)
        result["admission_diagnostics"] = admission
        self.assertEqual(self.observe(result), "INCOMPLETE")

    def test_policy_change_is_incomplete(self):
        self.policy.write_text('{"changed": true}', encoding="utf-8")
        self.assertEqual(self.observe(copy.deepcopy(self.fixture)), "INCOMPLETE")

    def test_unexpected_content_is_incomplete(self):
        result = copy.deepcopy(self.fixture)
        result.update(marker_exists=True, marker_content="not the proposed effect")
        self.assertEqual(self.observe(result), "INCOMPLETE")


if __name__ == "__main__":
    unittest.main()
