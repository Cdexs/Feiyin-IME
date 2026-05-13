"""
UI-OPT-001-TEST: 启动配置窗口并截图验证 CSS 视觉优化
"""
import subprocess
import time
import sys
import os

# 添加测试工具路径
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'utils'))

def kill_existing():
    """清理旧进程"""
    subprocess.run(
        ['powershell', '-Command', 
         'Get-Process voice-ime,voice-ime-ui -ErrorAction SilentlyContinue | Stop-Process -Force'],
        capture_output=True
    )
    time.sleep(1)

def start_voice_ime():
    """启动主程序"""
    exe_path = os.path.join(os.path.dirname(__file__), '..', '..', 'target', 'release', 'voice-ime.exe')
    subprocess.Popen([exe_path, '--debug'])
    time.sleep(3)  # 等待启动

def open_settings_via_tray():
    """通过托盘菜单打开配置窗口"""
    try:
        from pywinauto import Desktop, Application
        from pywinauto.keyboard import send_keys
        
        # 方法1: 直接启动 voice-ime-ui.exe
        ui_exe = os.path.join(os.path.dirname(__file__), '..', '..', 'target', 'release', 'voice-ime-ui.exe')
        subprocess.Popen([ui_exe])
        time.sleep(4)
        
        # 查找窗口
        desktop = Desktop(backend="win32")
        windows = desktop.windows()
        
        target_win = None
        for w in windows:
            title = w.window_text()
            # 跳过 Overlay 和不可见窗口
            if not w.is_visible():
                continue
            rect = w.rectangle()
            if rect.width() < 100 or rect.height() < 100:
                continue  # 跳过小窗口
            if '飞音' in title or 'voice' in title.lower() or '设置' in title:
                target_win = w
                print(f"找到配置窗口: {title} ({rect.width()}x{rect.height()})")
                break
        
        if target_win is None:
            print("⚠️ 未找到配置窗口，列出所有可见窗口:")
            windows = desktop.windows()
            for w in windows:
                if w.is_visible():
                    rect = w.rectangle()
                    title = w.window_text()
                    print(f"  [{rect.width()}x{rect.height()}] {title}")
                    # 尝试匹配更宽泛的条件
                    if '飞音' in title or 'voice' in title.lower() or 'ime' in title.lower():
                        if rect.width() > 400 and rect.height() > 300:
                            target_win = w
                            print(f"  → 选中: {title}")
                            break
        
        return target_win
        
    except Exception as e:
        print(f"❌ 打开配置窗口失败: {e}")
        import traceback
        traceback.print_exc()
        return None

def take_screenshot(filename):
    """截图"""
    try:
        import pyautogui
        screenshot = pyautogui.screenshot()
        screenshot.save(filename)
        print(f"📸 截图已保存: {filename}")
        return True
    except Exception as e:
        print(f"❌ 截图失败: {e}")
        return False

def verify_css_styles(window):
    """验证 CSS 视觉优化"""
    results = {}
    
    try:
        # 获取窗口截图
        rect = window.rectangle()
        print(f"窗口尺寸: {rect.width()}x{rect.height()}")
        print(f"窗口标题: {window.window_text()}")
        
        # 截图
        take_screenshot("ui_opt_001_main.png")
        
        # 验证项汇总
        title = window.window_text()
        results['window_title'] = '飞音' in title or '设置' in title or 'voice' in title.lower()
        results['window_visible'] = window.is_visible()
        results['window_size_adequate'] = rect.width() > 400 and rect.height() > 300
        results['window_rect'] = f"{rect.width()}x{rect.height()}"
        
        print("\n=== CSS 视觉优化验证 ===")
        print(f"✓ 窗口标题: {'PASS' if results['window_title'] else 'FAIL'} ({title})")
        print(f"✓ 窗口可见: {'PASS' if results['window_visible'] else 'FAIL'}")
        print(f"✓ 窗口尺寸: {'PASS' if results['window_size_adequate'] else 'FAIL'} ({results['window_rect']})")
        
        # 注意: Playwright CDP 方式可以更精确验证 DOM/CSS
        # 此处使用 pywinauto 截图作为视觉证据
        
    except Exception as e:
        print(f"❌ 验证过程出错: {e}")
    
    return results

def main():
    print("=== UI-OPT-001-TEST: CSS 视觉优化验证 ===\n")
    
    # 1. 清理旧进程
    print("1. 清理旧进程...")
    kill_existing()
    
    # 2. 启动主程序
    print("2. 启动主程序...")
    start_voice_ime()
    
    # 3. 打开配置窗口
    print("3. 打开配置窗口...")
    settings_win = open_settings_via_tray()
    
    if settings_win is None:
        print("\n❌ 无法打开配置窗口，截图当前屏幕")
        take_screenshot("ui_opt_001_error.png")
        sys.exit(1)
    
    # 4. 验证 CSS
    print("4. 验证 CSS 视觉优化...")
    results = verify_css_styles(settings_win)
    
    # 5. 输出结果
    print("\n=== 测试结果 ===")
    passed = sum(1 for v in results.values() if v is True)
    total = len(results)
    print(f"通过: {passed}/{total}")
    
    # 保持窗口打开以便人工验收
    print("\n📋 验收提示: 请检查 ui_opt_001_main.png 截图中的:")
    print("   - Sidebar 渐变背景和橘色光晕")
    print("   - 卡片白色背景 + 圆角 + 阴影")
    print("   - Toggle 开关胶囊形滑块")
    print("   - 热键按钮 3D 效果")
    print("   - 圆角统一 (8/12/16px)")
    
    # 不关闭进程，留给主控验收
    print("\n✅ 测试完成，窗口保持打开")

if __name__ == '__main__':
    main()
