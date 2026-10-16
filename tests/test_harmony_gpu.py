# SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
import subprocess
import unittest
from unittest.mock import patch

from harmony_gpu import configure


class HarmonyGpuTests(unittest.TestCase):
    def run_configure(self, replies, target=None):
        def reply(command, **kwargs):
            self.commands.append(command)
            return subprocess.CompletedProcess(command, 0, next(replies))

        self.commands = []
        with patch("harmony_gpu.subprocess.run", side_effect=reply):
            configure(target)

    def test_pinned_image_sets_and_verifies_property_on_selected_device(self):
        self.run_configure(iter([
            "OpenHarmony-7.0.0.39\r\n", "x86_64\n", "ohos\n",
            "Set parameter web.oop.gpu true success\n", "true \n",
        ]), "127.0.0.1:55555")
        self.assertTrue(all(c[:4] == ["hdc", "-t", "127.0.0.1:55555", "shell"]
                            for c in self.commands))
        self.assertEqual(self.commands[-2][-1], "param set web.oop.gpu true")
        self.assertEqual(self.commands[-1][-1], "param get web.oop.gpu")

    def test_other_images_and_physical_devices_are_unchanged(self):
        for image, arch in [("OpenHarmony-6.1.0.31", "x86_64"),
                            ("OpenHarmony-7.0.0.39", "aarch64")]:
            with self.subTest(image=image, arch=arch):
                self.run_configure(iter([image, arch]))
                self.assertEqual(len(self.commands), 2)
                self.assertTrue(all(c[:2] == ["hdc", "shell"] for c in self.commands))

    def test_transport_failure_with_zero_exit_status_is_fatal(self):
        with self.assertRaisesRegex(RuntimeError, "Fail"):
            self.run_configure(iter(["[Fail] No connected target"]))

    def test_failed_property_write_is_fatal_even_when_hdc_returns_zero(self):
        with self.assertRaisesRegex(RuntimeError, "Could not enable"):
            self.run_configure(iter([
                "OpenHarmony-7.0.0.39", "x86_64", "ohos", "permission denied", "false",
            ]))

    def test_model_that_ignores_property_is_rejected_before_write(self):
        with self.assertRaisesRegex(RuntimeError, "ignores web.oop.gpu"):
            self.run_configure(iter(["OpenHarmony-7.0.0.39", "x86_64", "emulator"]))
        self.assertEqual(len(self.commands), 3)
