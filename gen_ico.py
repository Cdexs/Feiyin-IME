import os
from PIL import Image

icons_dir = r'D:\Workspace\CodeLab\voice-ime\src-tauri\icons'
source = Image.open(os.path.join(icons_dir, 'icon-source.png'))

sizes = [(16,16), (32,32), (48,48), (64,64), (128,128), (256,256)]
resized = [source.resize(s, Image.Resampling.LANCZOS) for s in sizes]

ico_path = os.path.join(icons_dir, 'icon.ico')
resized[0].save(ico_path, format='ICO', sizes=sizes, append_images=resized[1:])
print(f'ICO saved with {len(sizes)} sizes')

# Verify file size
import os
size = os.path.getsize(ico_path)
print(f'File size: {size} bytes')
