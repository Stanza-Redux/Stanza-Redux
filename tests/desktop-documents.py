#!/usr/bin/env python3
"""Native macOS cold/warm file-open regression. Run after `day launch -p TARGET`.
Usage: python3 tests/desktop-documents.py macos-appkit [path/to/day]
Restarts only the Stanza build it tests.
"""
import hashlib
import json
import subprocess
import sys
import time
from pathlib import Path

root = Path(__file__).resolve().parents[1]
target = sys.argv[1]
assert target == 'macos-appkit'
day = sys.argv[2] if len(sys.argv) > 2 else 'day'
book = root / 'resource/assets/Alice.epub'  # Test fixture; production uses generated resources.
key = 'reader-' + hashlib.sha256(book.read_bytes()).hexdigest()
app = root / 'build/day/macos-appkit/Debug/StanzaRedux.app'
sessions = root / 'build/day/sessions.json'
entry = next(e for e in json.loads(sessions.read_text()) if e['target'] == target)
subprocess.run([day, 'stop', '-p', target], cwd=root, check=True)
subprocess.run(['open', '-n', '--env', 'DAYSCRIPT_PORT=' + str(entry['enginePort']),
                '--env', 'DAYSCRIPT_TOKEN=' + entry['engineToken'], '-a', str(app), str(book)], check=True)
entries = [e for e in json.loads(sessions.read_text()) if e['target'] != target]
entries.append(entry)
sessions.write_text(json.dumps(entries))
time.sleep(3)

def drive(label, steps):
    result = subprocess.run([day, 'drive', '-p', target, '--steps-json', json.dumps(steps)],
                            cwd=root, capture_output=True, text=True)
    text = result.stdout
    report = json.loads(text[text.find('{'):])
    failures = [s for s in report['steps'] if not s.get('ok')]
    print(target, label, 'failures:', failures, flush=True)
    assert result.returncode == 0 and not failures

ready = [
    {'wait_for': {'id': 'book-webview', 'timeout_secs': 30}},
    {'web_eval': {'id': 'book-webview', 'script': "String(stanza.state().ready && stanza.state().title.includes('Alice'))",
                  'text': 'true', 'timeout_secs': 30}},
]
drive('cold Launch Services delivery', ready)
subprocess.run(['open', '-a', str(app), str(book)], check=True)
time.sleep(2)
drive('warm delivery', ready)
# Closing the one stable window must leave no duplicate reader behind.
drive('single reader after repeated open', [
    {'close_window': {'window': key}},
    {'assert_missing': {'id': 'book-webview'}},
])
subprocess.run(['open', '-a', str(app), str(book)], check=True)
drive('reopen saved library book', ready)
