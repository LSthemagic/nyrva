"""Read-only Windows executable/import diagnostics for the isolated CI smoke.
Does not install runtimes, replace DLLs, modify PATH, or access provider data.
"""
import ctypes
import json
import os
from pathlib import Path
import sys
import pefile

exe = Path(sys.argv[1]).resolve()
kernel = ctypes.WinDLL('kernel32', use_last_error=True)
kernel.LoadLibraryExW.argtypes = [ctypes.c_wchar_p, ctypes.c_void_p, ctypes.c_uint32]
kernel.LoadLibraryExW.restype = ctypes.c_void_p
kernel.GetProcAddress.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
kernel.GetProcAddress.restype = ctypes.c_void_p
kernel.GetModuleFileNameW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_uint32]
kernel.GetModuleFileNameW.restype = ctypes.c_uint32
kernel.FreeLibrary.argtypes = [ctypes.c_void_p]
kernel.SetErrorMode(0x0001 | 0x0002 | 0x8000)
report = {'exe': str(exe), 'imports': []}
pe = pefile.PE(str(exe))
for entry in getattr(pe, 'DIRECTORY_ENTRY_IMPORT', []):
    name = entry.dll.decode('ascii')
    local = exe.parent / name
    target = str(local) if local.is_file() else name
    # Inspect exports without running imported library initialization code.
    handle = kernel.LoadLibraryExW(target, None, 1)
    row = {'dll': name, 'loaded_for_inspection': bool(handle)}
    if not handle:
        row['error'] = ctypes.get_last_error()
    else:
        buffer = ctypes.create_unicode_buffer(32768)
        kernel.GetModuleFileNameW(handle, buffer, len(buffer))
        row['resolved'] = buffer.value
        missing = []
        for symbol in entry.imports:
            ptr = ctypes.cast(ctypes.c_char_p(symbol.name), ctypes.c_void_p) if symbol.name else ctypes.c_void_p(symbol.ordinal)
            if not kernel.GetProcAddress(handle, ptr):
                missing.append(symbol.name.decode('ascii') if symbol.name else f'ordinal:{symbol.ordinal}')
        row['missing_exports'] = missing
        kernel.FreeLibrary(handle)
    report['imports'].append(row)
pe.close()
Path('experience-evidence').mkdir(exist_ok=True)
Path('experience-evidence/loader.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
print(json.dumps(report, indent=2))
