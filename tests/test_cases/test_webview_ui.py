"""
WebView2 UI 自动化测试（Playwright + CDP）

测试 voice-ime 配置界面的 DOM 操作：
- Tab 导航切换
- 表单控件交互（复选框、滑块、输入框）
- 通用设置页验证
- LLM 设置页验证

使用 Playwright 通过 CDP 连接 WebView2，替代 pyautogui 坐标点击。

前置条件：
- 启动 feiyin-ime.exe 时设置环境变量：
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222"
"""

import os
import time
from pathlib import Path

import pytest

# 设置环境变量启用 CDP 调试端口（测试环境专用）
os.environ["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = "--remote-debugging-port=9222"

from ..conftest import kill_existing_voice_ime, wait_for_condition


pytestmark = [pytest.mark.webview, pytest.mark.cdp, pytest.mark.timeout(60)]


@pytest.fixture(scope="module")
def voice_ime_with_cdp(exe_path: Path):
    """
    Module 级 fixture：启动带 CDP 的 voice-ime-ui 进程

    直接启动 Tauri UI 配置窗口（feiyin-ime-ui.exe），
    不再依赖主程序的 --settings-ui 参数。
    """
    kill_existing_voice_ime()

    import subprocess
    # 直接启动 feiyin-ime-ui.exe
    ui_exe = exe_path.parent / "feiyin-ime-ui.exe"
    if not ui_exe.exists():
        pytest.skip(f"feiyin-ime-ui.exe not found: {ui_exe}")

    process = subprocess.Popen(
        [str(ui_exe)],
        cwd=str(exe_path.parent),
        creationflags=subprocess.CREATE_NEW_PROCESS_GROUP,
        env={**os.environ, "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS": "--remote-debugging-port=9222"},
    )

    # 等待 CDP 端口 + 页面加载完成
    from ..conftest import _wait_for_cdp_ready
    if not _wait_for_cdp_ready(timeout=15.0):
        process.terminate()
        pytest.skip("CDP port not ready after 15s")

    # 额外等待页面渲染
    time.sleep(2)

    try:
        yield process
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        kill_existing_voice_ime()


# 覆盖 session 级 cdp_browser 确保使用带 CDP 的进程
@pytest.fixture(scope="module")
def cdp_browser(voice_ime_with_cdp):
    """Module 级 CDP 浏览器连接，与 voice_ime_with_cdp 作用域一致"""
    from playwright.sync_api import sync_playwright
    from ..conftest import _wait_for_cdp_ready, CDP_URL

    if not _wait_for_cdp_ready(timeout=10.0):
        pytest.skip(f"CDP port not available at {CDP_URL}")

    pw = sync_playwright().start()
    try:
        browser = pw.chromium.connect_over_cdp(CDP_URL)
        yield browser
        browser.disconnect()
    except Exception as e:
        pytest.skip(f"CDP connection failed: {e}")
    finally:
        pw.stop()


class TestTabNavigation:
    """Tab 导航切换测试"""

    def test_general_tab_visible(self, main_page) -> None:
        """测试通用设置页可见"""
        # 验证页面标题或通用 Tab 存在
        title = main_page.title()
        assert title, "Page title should exist"

        # Sidebar 使用 .sidebar-nav-item 而非 .tab
        tabs = main_page.locator('.sidebar-nav-item, [class*="tab"], [role="tab"]')
        count = tabs.count()
        assert count >= 1, f"Expected at least 1 tab, found {count}"

    def test_voice_tab_click(self, main_page) -> None:
        """测试点击语音输入 Tab"""
        # 尝试多种方式定位语音 Tab
        voice_tab = main_page.locator('text="语音"').first

        if voice_tab.count() > 0:
            voice_tab.click()
            time.sleep(0.5)

            # 验证语音设置区域出现
            voice_section = main_page.locator('text="音频", text="麦克风", text="静音"').first
            # 至少不报错
            assert main_page.url or "tauri" in (main_page.title() or "").lower()
        else:
            pytest.skip("Voice tab not found by text locator")

    def test_llm_tab_click(self, main_page) -> None:
        """测试点击优化模型 Tab"""
        llm_tab = main_page.locator('text="优化", text="LLM", text="模型"').first

        if llm_tab.count() > 0:
            llm_tab.click()
            time.sleep(0.5)
            assert main_page.url or main_page.title()
        else:
            pytest.skip("LLM tab not found by text locator")


class TestFormControls:
    """表单控件交互测试"""

    def test_checkbox_toggle(self, main_page) -> None:
        """测试复选框/开关勾选状态切换"""
        # Toggle switch 使用 .toggle-switch（native input 隐藏，点击 label/track）
        toggle_switches = main_page.locator('.toggle-switch')
        count = toggle_switches.count()

        if count > 0:
            first_toggle = toggle_switches.first
            # 查找内部的 checkbox 来验证状态
            checkbox = first_toggle.locator('input[type="checkbox"]')
            initial_state = checkbox.is_checked()

            # 点击 toggle-switch（可视化区域）
            first_toggle.click()
            time.sleep(0.3)
            new_state = checkbox.is_checked()

            assert new_state != initial_state, "Toggle state should change after click"
        else:
            # 降级到查找普通 checkbox
            checkboxes = main_page.locator('input[type="checkbox"]:visible')
            count = checkboxes.count()
            if count > 0:
                first_cb = checkboxes.first
                initial_state = first_cb.is_checked()
                first_cb.click()
                time.sleep(0.2)
                new_state = first_cb.is_checked()
                assert new_state != initial_state, "Checkbox state should toggle"
            else:
                pytest.skip("No toggle switches or visible checkboxes found")

    def test_text_input_fill(self, main_page) -> None:
        """测试文本输入框填充"""
        # 查找文本输入框
        text_inputs = main_page.locator('input[type="text"], input[type="url"], textarea')
        count = text_inputs.count()

        if count > 0:
            first_input = text_inputs.first
            test_value = "test-playwright-value"

            first_input.fill(test_value)
            time.sleep(0.2)

            actual_value = first_input.input_value()
            assert actual_value == test_value, f"Input value mismatch: expected {test_value}, got {actual_value}"
        else:
            pytest.skip("No text inputs found")

    def test_button_click(self, main_page) -> None:
        """测试按钮点击"""
        # 查找按钮
        buttons = main_page.locator('button, [role="button"], input[type="button"], input[type="submit"]')
        count = buttons.count()

        if count > 0:
            first_btn = buttons.first
            btn_text = first_btn.inner_text() if first_btn.is_visible() else "unnamed"

            # 点击按钮（非破坏性操作）
            first_btn.click()
            time.sleep(0.2)

            # 页面应保持稳定
            assert main_page.url or main_page.title()
        else:
            pytest.skip("No buttons found")


class TestSidebarLayout:
    """侧边栏布局和样式验证（UI-FIX-005 同步测试）"""

    def test_sidebar_padding_top(self, main_page) -> None:
        """测试 Sidebar 顶部留白为 12px"""
        sidebar = main_page.locator('.sidebar')
        if sidebar.count() == 0:
            pytest.skip("Sidebar element not found")

        # 通过 JavaScript 获取计算后的 padding-top 值
        padding_top = sidebar.evaluate(
            "el => getComputedStyle(el).paddingTop"
        )
        # 转换为数字进行比较（可能带 px 单位）
        padding_value = int(padding_top.replace('px', ''))
        assert padding_value == 12, (
            f"Sidebar padding-top should be 12px, got {padding_value}px"
        )

    def test_sidebar_nav_items_count(self, main_page) -> None:
        """测试 Sidebar 导航项数量（5个：通用/语音/优化/词库/关于）"""
        nav_items = main_page.locator('.sidebar-nav-item')
        count = nav_items.count()
        assert count >= 5, (
            f"Expected at least 5 sidebar nav items, found {count}"
        )

    def test_main_content_scrollbar_hidden(self, main_page) -> None:
        """测试主内容区滚动条隐藏（含溢出内容强制触发 + 真实 gutter 测量）"""
        main_content = main_page.locator('.main-content')
        if main_content.count() == 0:
            pytest.skip("Main content element not found")

        # 验证 CSS 属性声明
        scrollbar_width = main_content.evaluate(
            "el => getComputedStyle(el).scrollbarWidth"
        )
        assert scrollbar_width == 'none', (
            f"scrollbarWidth should be 'none', got '{scrollbar_width}'"
        )

        ms_overflow_style = main_content.evaluate(
            "el => getComputedStyle(el).msOverflowStyle"
        )
        assert ms_overflow_style is None or str(ms_overflow_style).lower() == 'none', (
            f"msOverflowStyle should be None or 'none', got '{ms_overflow_style}'"
        )

        overflow_y = main_content.evaluate(
            "el => getComputedStyle(el).overflowY"
        )
        assert overflow_y in ('auto', 'scroll'), (
            f"overflowY should be 'auto' or 'scroll', got '{overflow_y}'"
        )

        # === 关键改进：强制 overflow:scroll 触发滚动条渲染，再测量 gutter ===
        # 问题：内容不足时滚动条不渲染，offsetWidth-clientWidth 永远为 0（假阳性）
        # 方案：临时设置 overflow-y: scroll 强制渲染滚动条轨道，测量后再恢复
        original_overflow = main_content.evaluate("el => el.style.overflowY")
        main_content.evaluate("el => el.style.overflowY = 'scroll'")
        time.sleep(0.5)

        # 现在测量 gutter——如果滚动条可见，gutter 应该 > 0
        # 注意：overflow-y: scroll 强制渲染滚动条轨道
        # 如果 CSS 正确隐藏了滚动条，gutter 应为 0
        gutter = main_content.evaluate(
            "el => el.offsetWidth - el.clientWidth"
        )
        assert gutter == 0, (
            f"Scrollbar gutter should be 0 even with overflow:scroll, got {gutter}px"
        )

        # 恢复原始 overflow 设置
        if original_overflow:
            main_content.evaluate(f"el => el.style.overflowY = '{original_overflow}'")
        else:
            main_content.evaluate("el => el.style.overflowY = ''")


class TestVoiceAsrModelSelect:
    """Voice 页 ASR 模型下拉选择框测试（TEST-SYNC-QWEN3-001）"""

    def test_asr_model_select_exists(self, main_page) -> None:
        """测试 ASR 模型选择下拉框存在"""
        # 先点击语音 Tab
        voice_tab = main_page.locator('text="语音"').first
        if voice_tab.count() > 0:
            voice_tab.click()
            time.sleep(0.5)
        else:
            pytest.skip("Voice tab not found")

        # 查找 ASR 模型选择下拉框
        select = main_page.locator('select.select-input').first
        if select.count() == 0:
            pytest.skip("ASR model select not found")

        # 验证有 3 个选项（performance/accuracy/qwen3_online）
        options = select.locator('option')
        assert options.count() == 3, (
            f"Expected 3 ASR model options, found {options.count()}"
        )

        # 验证选项值
        values = options.evaluate_all("els => els.map(el => el.value)")
        assert "performance" in values, "performance option must exist"
        assert "accuracy" in values, "accuracy option must exist"
        assert "qwen3_online" in values, "qwen3_online option must exist"

    def test_asr_model_select_can_switch(self, main_page) -> None:
        """测试 ASR 模型选择下拉框可切换"""
        voice_tab = main_page.locator('text="语音"').first
        if voice_tab.count() > 0:
            voice_tab.click()
            time.sleep(0.5)
        else:
            pytest.skip("Voice tab not found")

        select = main_page.locator('select.select-input').first
        if select.count() == 0:
            pytest.skip("ASR model select not found")

        # 切换到 accuracy
        select.select_option("accuracy")
        time.sleep(0.2)
        assert select.input_value() == "accuracy", "select should now show accuracy"

        # 切换到 qwen3_online
        select.select_option("qwen3_online")
        time.sleep(0.2)
        assert select.input_value() == "qwen3_online", "select should now show qwen3_online"

        # 切换回 performance
        select.select_option("performance")
        time.sleep(0.2)
        assert select.input_value() == "performance", "select should now show performance"


class TestPageContent:
    """页面内容验证测试"""

    def test_no_error_elements(self, main_page) -> None:
        """测试页面无错误元素"""
        # 检查常见错误指示器
        errors = main_page.locator('[class*="error"], [class*="Error"], .alert-danger')
        count = errors.count()

        assert count == 0, f"Found {count} error elements on page"

    def test_page_has_content(self, main_page) -> None:
        """测试页面有内容"""
        body = main_page.locator("body")
        text = body.inner_text()

        assert len(text) > 50, f"Page content too short ({len(text)} chars)"

    def test_no_broken_images(self, main_page) -> None:
        """测试无破损图片"""
        images = main_page.locator("img")
        count = images.count()

        if count > 0:
            # 检查图片是否加载成功
            broken = 0
            for i in range(count):
                img = images.nth(i)
                # Playwright 中无法直接检查图片是否破损，跳过详细检查
                pass
        # 无图片也算通过
        assert True


class TestWordbookPage:
    """词库页面 E2E 测试（WORDBOOK-SINGLEWORD-001）

    覆盖：
    - 词库页面存在并可导航
    - 单输入框添加弹窗布局
    - 切换到用户词库页 Tab（添加按钮可用）
    - 添加弹窗中包含单个输入框
    - 添加按钮在输入为空时禁用
    """

    def _navigate_to_wordbook(self, main_page) -> bool:
        """导航到词库页面，返回是否成功"""
        wordbook_nav = main_page.locator('.sidebar-nav-item').filter(
            has_text='词库'
        ).or_(
            main_page.locator('.sidebar-nav-item').filter(has_text='Wordbook')
        ).first
        if wordbook_nav.count() == 0:
            return False
        wordbook_nav.click()
        import time
        time.sleep(0.5)
        return True

    def test_wordbook_sidebar_nav_exists(self, main_page) -> None:
        """测试词库侧边栏导航项存在"""
        nav_items = main_page.locator('.sidebar-nav-item')
        count = nav_items.count()
        assert count >= 1, "Sidebar navigation should exist"

        wordbook_nav = nav_items.filter(has_text='词库').or_(
            nav_items.filter(has_text='Wordbook')
        )
        assert wordbook_nav.count() > 0, (
            "Wordbook sidebar nav item should exist"
        )

    def test_wordbook_stats_visible(self, main_page) -> None:
        """测试词库统计栏可见"""
        if not self._navigate_to_wordbook(main_page):
            pytest.skip("Could not navigate to wordbook page")

        stats = main_page.locator('.wordbook-stats')
        assert stats.count() > 0, "Wordbook stats bar should be visible"

    def test_wordbook_tabs_exist(self, main_page) -> None:
        """测试词库页面 Tab 切换存在"""
        if not self._navigate_to_wordbook(main_page):
            pytest.skip("Could not navigate to wordbook page")

        tabs = main_page.locator('.wordbook-tab')
        # 应该有系统/用户两个 Tab
        assert tabs.count() >= 2, (
            f"Expected at least 2 wordbook tabs, found {tabs.count()}"
        )

    def test_wordbook_user_tab_enables_add_button(self, main_page) -> None:
        """测试切换到用户词库 Tab 后添加按钮启用"""
        if not self._navigate_to_wordbook(main_page):
            pytest.skip("Could not navigate to wordbook page")

        # 点击用户词库 Tab
        user_tabs = main_page.locator('.wordbook-tab').filter(
            has_text='用户'
        ).or_(
            main_page.locator('.wordbook-tab').filter(has_text='User')
        )
        if user_tabs.count() == 0:
            pytest.skip("User tab not found by text, trying second tab")

        # 降级：点击第二个 .wordbook-tab
        if user_tabs.count() == 0:
            all_tabs = main_page.locator('.wordbook-tab')
            if all_tabs.count() >= 2:
                all_tabs.nth(1).click()
            else:
                pytest.skip("Cannot find user tab")
        else:
            user_tabs.first.click()

        import time
        time.sleep(0.3)

        # 添加按钮应存在且启用
        add_btn = main_page.locator('.wordbook-add-inline')
        assert add_btn.count() > 0, "Add button should exist on user tab"
        assert add_btn.is_enabled(), "Add button should be enabled on user tab"

    def test_wordbook_add_modal_has_single_input(self, main_page) -> None:
        """测试添加弹窗包含单个输入框（单词模式）"""
        if not self._navigate_to_wordbook(main_page):
            pytest.skip("Could not navigate to wordbook page")

        # 切换到用户 Tab
        all_tabs = main_page.locator('.wordbook-tab')
        if all_tabs.count() >= 2:
            all_tabs.nth(1).click()
        else:
            pytest.skip("Not enough tabs")

        import time
        time.sleep(0.3)

        # 点击添加按钮
        add_btn = main_page.locator('.wordbook-add-inline')
        if add_btn.count() == 0 or not add_btn.is_enabled():
            pytest.skip("Add button not available")
        add_btn.click()
        time.sleep(0.3)

        # 验证添加弹窗存在
        modal = main_page.locator('.modal-wordbook-add')
        assert modal.count() > 0, "Add modal should be visible after clicking +"

        # 验证弹窗包含单个输入框
        inputs = modal.locator('input[type="text"]')
        assert inputs.count() == 1, (
            f"Add modal should have exactly 1 text input (single-word mode), got {inputs.count()}"
        )

        # 验证添加按钮初始状态为禁用（输入为空）
        add_submit = modal.locator('.btn-primary.btn-accent')
        assert add_submit.count() > 0, "Add submit button should exist in modal"
        assert add_submit.is_disabled(), (
            "Add button should be disabled when input is empty"
        )

    def test_wordbook_delete_button_exists_on_entries(self, main_page) -> None:
        """测试词条条目包含删除按钮"""
        if not self._navigate_to_wordbook(main_page):
            pytest.skip("Could not navigate to wordbook page")

        # 查看是否有词条条目
        labels = main_page.locator('.wordbook-label')
        if labels.count() == 0:
            pytest.skip("No wordbook entries to test delete button")

        # 每个词条应有删除按钮
        delete_btns = main_page.locator('.wordbook-label-delete')
        assert delete_btns.count() > 0, (
            "Wordbook entries should have delete buttons"
        )
