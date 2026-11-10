# Synthetic 1PUX archive fixtures

Run `python src-tauri/tests/fixtures/onepux/generate.py` from the repository root
to regenerate the five deterministic, sub-2-KiB archives. Python's standard ZIP
writer independently supplies stored, deflate, forced local ZIP64 and duplicate
members. The script adds a small ZIP64 end record explicitly. Metadata has no
accounts or credentials; files contain fixed synthetic bytes, an empty binary,
and a nested UTF-8 filename. Nothing comes from a real vault or client export.

The Rust tests mutate these small samples to check framing, CRC, member identity
and resource rejection. They do not establish actual 1Password export or Android
document-provider interoperability, item migration or secure allocator erasure.
