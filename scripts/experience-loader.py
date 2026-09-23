"""Inspect native smoke resources and imports without executing imported DLLs.
The Python host has its own activation context; inspecting its loaded libraries
alone cannot prove which comctl32 version the separate smoke executable selects.
"""
import json
import os
from pathlib import Path
import sys
import pefile

exe = Path(sys.argv[1]).resolve()
pe = pefile.PE(str(exe))
manifests = []
for resource in getattr(getattr(pe, 'DIRECTORY_ENTRY_RESOURCE', None), 'entries', []):
    if resource.id == 24:  # RT_MANIFEST
        for name in resource.directory.entries:
            for language in name.directory.entries:
                item = language.data.struct
                manifests.append(pe.get_data(item.OffsetToData, item.Size).decode('utf-8', errors='replace'))
controls = [entry for entry in getattr(pe, 'DIRECTORY_ENTRY_IMPORT', []) if entry.dll.decode('ascii').lower() == 'comctl32.dll']
system_dll = Path(os.environ['SystemRoot']) / 'System32' / 'comctl32.dll'
system_pe = pefile.PE(str(system_dll))
exports = getattr(getattr(system_pe, 'DIRECTORY_ENTRY_EXPORT', None), 'symbols', [])
names = {symbol.name for symbol in exports}
ordinals = {symbol.ordinal for symbol in exports}
missing = []
for entry in controls:
    for symbol in entry.imports:
        if (symbol.name not in names) if symbol.name else (symbol.ordinal not in ordinals):
            missing.append(symbol.name.decode('ascii') if symbol.name else f'ordinal:{symbol.ordinal}')
report = {
    'exe': str(exe),
    'embedded_manifests': manifests,
    'common_controls_v6': any('Microsoft.Windows.Common-Controls' in m and '6.0.0.0' in m for m in manifests),
    'system_comctl32': str(system_dll),
    'imports_missing_in_system_comctl32': missing,
    'note': 'A v6 activation context is required; the system32 DLL is not a substitute. No DLLs or PATH were modified.',
}
pe.close()
system_pe.close()
Path('experience-evidence').mkdir(exist_ok=True)
Path('experience-evidence/loader.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
print(json.dumps(report, indent=2))
assert report['common_controls_v6'], 'native smoke lacks its required embedded Common Controls v6 manifest'
