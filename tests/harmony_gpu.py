#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
"""Isolate ArkWeb's GPU on the OpenHarmony image used by CI, before app launch."""

import os
import subprocess


def configure(target=None):
    hdc = ["hdc"] + (["-t", target] if target else [])

    def shell(command):
        result = subprocess.run(
            [*hdc, "shell", command], check=True, text=True,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            timeout=30,
        ).stdout.strip()
        # HDC can report transport errors while returning exit status zero.
        if "[Fail]" in result:
            raise RuntimeError(result)
        return result

    image = shell("param get const.ohos.fullname")
    arch = shell("uname -m")
    if (image, arch) != ("OpenHarmony-7.0.0.39", "x86_64"):
        print(f"Leaving ArkWeb GPU configuration unchanged for {image} ({arch})")
        return

    # This runtime forces in-process GPU when the product model is "emulator",
    # regardless of the property. The pinned ohos-qemu image's model is "ohos".
    if shell("param get const.product.model") == "emulator":
        raise RuntimeError("This emulator model ignores web.oop.gpu")
    # Avoid Chrome_InProcGp sharing the app process with RSRenderThread: the
    # pinned Mesa driver intermittently aborts on !draw->flushing in draw_flush.
    # This is a nonpersistent guest setting; apply after each boot, before launch.
    shell("param set web.oop.gpu true")
    if shell("param get web.oop.gpu") != "true":
        raise RuntimeError("Could not enable ArkWeb's separate GPU process")
    print("ArkWeb GPU process enabled (web.oop.gpu=true)")


if __name__ == "__main__":
    configure(os.environ.get("DAY_OHOS_TARGET"))
