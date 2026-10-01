# Screenshot books

Unmodified Project Gutenberg EPUBs, downloaded 2026-10-01. The texts by Mary Shelley
and H. G. Wells are in the public domain; the EPUBs retain Gutenberg’s license and
credits. `provenance.json` records the source URLs and SHA-256 hashes. Alice uses the
existing bundled edition in `resource/assets/Alice.epub`.

These files are served only by the loopback CI fixture. They are not bundled in the
app. `dayscript/store-walkthrough.yaml` acquires the real books through OPDS, then
captures the reader, library, and settings. It needs no public network connection.
The fixture’s “Public-domain classics” catalog is a demonstration collection, not
an imitation of a live service. Publication titles and excerpts remain in English;
the app’s controls and gallery titles follow the selected locale.

Keep each EPUB in this fixture directory below 1 MiB. The release tests enforce this
limit so illustrated editions do not silently add large binaries to Git history.
