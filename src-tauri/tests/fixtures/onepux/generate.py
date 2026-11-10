"""Generate tiny, synthetic ZIP fixtures with Python's independent ZIP writer."""

import io
import pathlib
import struct
import warnings
import zipfile

ROOT = pathlib.Path(__file__).resolve().parent
MEMBERS = [
    ("export.attributes", b'{"version":3,"description":"synthetic fixture"}'),
    ("export.data", b'{"accounts":[]}'),
    ("files/", b""),
    ("files/blob__note.txt", b"synthetic attachment\x00\xff"),
    ("files/empty__empty.bin", b""),
    ("files/nested/笔记.txt", "合成附件".encode()),
]


def archive(compression, force_zip64=False, duplicate=False):
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", compression=compression) as writer:
        for name, content in MEMBERS + (MEMBERS[:1] if duplicate else []):
            info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
            info.compress_type = compression
            info.create_system = 3
            info.external_attr = (0o40755 if name.endswith("/") else 0o100600) << 16
            with writer.open(info, "w", force_zip64=force_zip64) as member:
                member.write(content)
    return buffer.getvalue()


ROOT.joinpath("stored.1pux").write_bytes(archive(zipfile.ZIP_STORED))
ROOT.joinpath("deflated.1pux").write_bytes(archive(zipfile.ZIP_DEFLATED))
local64 = archive(zipfile.ZIP_STORED, force_zip64=True)
ROOT.joinpath("local-zip64.1pux").write_bytes(local64)
# An actual ZIP64 end record for the same tiny archive; no large allocation.
end = len(local64) - 22
count = struct.unpack_from("<H", local64, end + 10)[0]
size, offset = struct.unpack_from("<II", local64, end + 12)
record = struct.pack("<4sQHHIIQQQQ", b"PK\x06\x06", 44, 45, 45, 0, 0, count, count, size, offset)
locator = struct.pack("<4sIQI", b"PK\x06\x07", 0, end, 1)
eocd = bytearray(local64[end:])
struct.pack_into("<HHII", eocd, 8, 0xFFFF, 0xFFFF, 0xFFFFFFFF, 0xFFFFFFFF)
ROOT.joinpath("directory-zip64.1pux").write_bytes(local64[:end] + record + locator + eocd)
with warnings.catch_warnings():
    warnings.simplefilter("ignore", UserWarning)
    ROOT.joinpath("duplicate.1pux").write_bytes(archive(zipfile.ZIP_STORED, duplicate=True))
