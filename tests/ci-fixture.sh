#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
# Source after the CI device boots, in the shell that runs the dayscripts.
set -euo pipefail
STANZA_PYTHON=python3
if ! "$STANZA_PYTHON" -c 'import sys' 2>/dev/null; then STANZA_PYTHON=python; fi
export STANZA_FIXTURE_PORT="${STANZA_FIXTURE_PORT:-18765}"
STANZA_FIXTURE_LOG="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/stanza-catalog-fixture.log"
export STANZA_FIXTURE_TOKEN="stanza-$$-$RANDOM"
"$STANZA_PYTHON" -u tests/catalog_server.py > "$STANZA_FIXTURE_LOG" 2>&1 &
STANZA_FIXTURE_PID=$!
stanza_fixture_cleanup() {
    local result=$?
    if [ "$result" != 0 ]; then tail -60 "$STANZA_FIXTURE_LOG" >&2 || true; fi
    kill "$STANZA_FIXTURE_PID" 2>/dev/null || true
    wait "$STANZA_FIXTURE_PID" 2>/dev/null || true
}
trap stanza_fixture_cleanup EXIT
"$STANZA_PYTHON" tests/prepare_fixture.py
case "${DAY_SCRIPT_TARGET:-}" in
    android-mdc)
        STANZA_ADB="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}/platform-tools/adb"
        if [ ! -x "$STANZA_ADB" ]; then STANZA_ADB=adb; fi
        "$STANZA_ADB" ${ANDROID_SERIAL:+-s "$ANDROID_SERIAL"} reverse "tcp:$STANZA_FIXTURE_PORT" "tcp:$STANZA_FIXTURE_PORT"
        ;;
    harmony-arkui)
        STANZA_FORWARD=$(hdc ${DAY_OHOS_TARGET:+-t "$DAY_OHOS_TARGET"} rport "tcp:$STANZA_FIXTURE_PORT" "tcp:$STANZA_FIXTURE_PORT")
        echo "$STANZA_FORWARD"
        # hdc can report failure on stdout with exit status zero.
        case "$STANZA_FORWARD" in *"[Fail]"*) exit 1 ;; esac
        ;;
esac
