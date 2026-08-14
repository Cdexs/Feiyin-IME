import re
import codecs

path = r'D:\Workspace\CodeLab\voice-ime\target\release\debug.log'
with codecs.open(path, 'r', 'utf-8') as f:
    content = f.read()

# Find the 10:47 scene F4 block
m = re.search(r'2026-08-03T10:47:.*?Scene F4 block injected: "(.*?)"', content, re.DOTALL)
if m:
    scene_text = m.group(1).replace('\\n', '\n')
    out_path = r'D:\Workspace\CodeLab\voice-ime\collab\research\scene_f4_document.txt'
    with codecs.open(out_path, 'w', 'utf-8') as out:
        out.write(scene_text)
    print('Length:', len(scene_text))
    print('Saved to:', out_path)
else:
    print('Not found')
