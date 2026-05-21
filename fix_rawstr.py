src = r"G:\oskarlolpo project\minecraftjava\01_Active\p2p\src-tauri\src\main.rs"

with open(src, 'r', encoding='utf-8') as f:
    lines = f.readlines()

# Lines 775 and 790 (0-indexed: 774, 789) have raw strings ending with #; but missing "
for idx in [774, 789]:
    line = lines[idx]
    stripped = line.rstrip('\n').rstrip('\r')
    if stripped.endswith('#;') and not stripped.endswith('"#;'):
        # Insert " before #;
        new_line = stripped[:-2] + '"#;\n'
        lines[idx] = new_line
        print(f'Fixed line {idx+1}: ends with ...{repr(new_line[-15:])}')
    else:
        print(f'Line {idx+1} OK or already fixed: ends with {repr(stripped[-10:])}')

with open(src, 'w', encoding='utf-8') as f:
    f.writelines(lines)

print('File saved.')
