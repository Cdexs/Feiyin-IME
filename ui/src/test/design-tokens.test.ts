// UITEST-137 · 设计令牌合规护栏（第一阶段，零基建改动，不需要浏览器）
//
// 背景：当前 vitest 环境是 happy-dom —— 没有 CSS 级联引擎、没有布局引擎，
// getComputedStyle 解析不了样式表规则，元素没有真实尺寸。所以颜色/宽度在这个
// 环境里**测不出来**（UITEST-138 上真实浏览器才解决）。本文件做的是不需要
// 浏览器就能拿到的收益：把「令牌体系本身的纪律」变成机器判据（源码级扫描，
// 在 Node 侧读文件，不依赖 DOM 渲染）。
//
// 三条护栏：
//   G1 硬编码色值禁令 —— 裸十六进制色值必须走 var(--token) 或 class。
//      存量进 BASELINE_WHITELIST（每条注明 文件:行 + 待迁移理由），增量直接红。
//   G2 令牌完整性 —— 每一个被引用的 var(--token) 都必须在 styles.css 有定义。
//      浏览器里引用未定义令牌是静默失效，肉眼才看得出来，只能在这里拦。
//   G3 令牌值唯一性 —— styles.css 里不允许两个不同令牌拥有完全相同的色值，
//      除非在 ALIAS_GROUPS 显式登记 alias 关系（防令牌体系长出重复分叉）。
//
// 另有一组行为测试（user-event + aria 断言）作为后续批次的范式示范：
// 断言「状态转换」与「可访问性属性」（checked / role="dialog"），
// 不断言 class 名 —— class 改名测试就红（脆），aria 编码的是设计意图。
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { join, relative } from 'node:path';
import { render, screen } from '@testing-library/react';
import React from 'react';
import userEvent from '@testing-library/user-event';
import HotkeySettingsPage from '../pages/HotkeySettings.tsx';
import { zhHans } from '../i18n/zh-Hans';

const SRC_ROOT = join(process.cwd(), 'src');

function walkFiles(dir: string, out: string[] = []): string[] {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, entry.name);
    if (entry.isDirectory()) walkFiles(p, out);
    else out.push(p);
  }
  return out;
}

// 可扫文件：tsx/ts/css；排除测试文件（.test. / .spec.）与 styles.css 本身
// （styles.css 是令牌的**定义处**，色值写在那里是合法的；G3 管它的内部纪律）。
function guardTargets(): string[] {
  return walkFiles(SRC_ROOT).filter((f) => {
    if (!/\.(tsx|ts|css)$/.test(f)) return false;
    if (f.endsWith('styles.css')) return false;
    if (/\.test\.|\.spec\./.test(f)) return false;
    return true;
  });
}

function readLines(f: string): string[] {
  return readFileSync(f, 'utf8').split('\n');
}

const HEX_COLOR_RE = /#[0-9a-fA-F]{8}\b|#[0-9a-fA-F]{6}\b|#[0-9a-fA-F]{3}\b/g;
const VAR_REF_RE = /var\(\s*(--[A-Za-z0-9-]+)/g;
const TOKEN_DEF_RE = /(--[A-Za-z0-9-]+)\s*:/g;

// ---------------------------------------------------------------------------
// G1 基线白名单：key = 相对路径:行号:小写色值。
// 🔴 存量不阻塞、增量被挡住 —— 新增硬编码 = 白名单外命中 = 红。
// 白名单内每条都要写「待迁移」理由；清完一条删一条，最终目标是空表。
// ---------------------------------------------------------------------------
const BASELINE_WHITELIST: Record<string, string> = {
  // About.tsx 副标题/版本号 —— 中性灰文本，styles.css 无对应令牌。
  // 待迁移：新增 --system-text-muted（需与 616161/9d9d9d 族的层级关系一起定）。
  'src/pages/About.tsx:105:#6b7280': '待迁移：无对应灰阶令牌',
  'src/pages/About.tsx:108:#6b7280': '待迁移：同上',
  // 深绿（成功态文本）。待迁移：与 --status-success(#22c55e) 是不同明度，
  // 拿不准不动 —— 需要设计侧确认 166534/16a34a/22c55e 的层级关系后再收敛。
  'src/pages/About.tsx:114:#166534': '待迁移：成功态深绿，与 --status-success 不等值',
  'src/pages/About.tsx:124:#16a34a': '待迁移：成功态绿，与 --status-success 不等值',
};

describe('G1 · 硬编码色值禁令（UITEST-137）', () => {
  it('ui/src 不出现白名单之外的裸十六进制色值', () => {
    const violations: string[] = [];
    for (const f of guardTargets()) {
      const rel = relative(process.cwd(), f).replace(/\\/g, '/');
      readLines(f).forEach((line, i) => {
        const matches = line.match(HEX_COLOR_RE);
        if (!matches) return;
        for (const raw of matches) {
          const key = `${rel}:${i + 1}:${raw.toLowerCase()}`;
          if (!BASELINE_WHITELIST[key]) {
            violations.push(`${key} —— 白名单外的新硬编码，请改用 var(--token) 或 class`);
          }
        }
      });
    }
    expect(violations).toEqual([]);
  });

  it('白名单自身必须仍然命中（防「改了代码忘删白名单」造成假豁免）', () => {
    // 白名单条目对应的色值还在源文件里，才说明「待迁移」仍然成立；
    // 若清理完成，应把条目从白名单删掉，而不是让白名单空挂。
    const present = new Set<string>();
    for (const f of guardTargets()) {
      const rel = relative(process.cwd(), f).replace(/\\/g, '/');
      readLines(f).forEach((line, i) => {
        for (const raw of line.match(HEX_COLOR_RE) ?? []) {
          present.add(`${rel}:${i + 1}:${raw.toLowerCase()}`);
        }
      });
    }
    const stale = Object.keys(BASELINE_WHITELIST).filter((k) => !present.has(k));
    expect(stale).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// G2 令牌完整性
// ---------------------------------------------------------------------------
function collectTokenDefinitions(): Set<string> {
  const css = readFileSync(join(SRC_ROOT, 'styles.css'), 'utf8');
  const defs = new Set<string>();
  for (const m of css.matchAll(TOKEN_DEF_RE)) defs.add(m[1].toLowerCase());
  return defs;
}

// G2 基线白名单：被引用但未定义、且**带 fallback**的令牌（运行时不会裸失效）。
// 每条写明为什么暂不定义为令牌；清掉一条删一条。不带 fallback 的裸引用 = 直接红。
const ALLOWED_UNDEFINED: Record<string, string> = {
  // styles.css:933 range 滑杆进度值：var(--value, 50%)，值由使用方内联注入。
  // 当前仓库里 input[type="range"] 无任何 tsx 使用方（选择器是死 CSS），
  // 所以没有注入点 —— 定义一个没人写的 --value 反而制造僵尸令牌。待滑杆真正接入时再定。
  '--value': '带 fallback(50%)，进度值由使用方注入，当前无使用方',
  // styles.css:1352 wordbook-add-inline.disabled 的文字色。未定义 → color 在计算值阶段
  // 无效、回落继承色。补一个定义会**改变现有渲染**（继承色 → 新令牌色），
  // 在设计侧定值之前不补（「拿不准不动」同理）。
  '--system-text-disabled': '待定义：需设计侧定值，当前回落继承色',
};

describe('G2 · 令牌完整性（UITEST-137）', () => {
  it('每个被引用的 var(--token) 都在 styles.css 有定义（或登记白名单）', () => {
    const defs = collectTokenDefinitions();
    const missing = new Map<string, string[]>();
    for (const f of walkFiles(SRC_ROOT)) {
      // 引用侧包括 styles.css 自己（令牌可以引用令牌）；仍排除测试文件
      if (f.endsWith('styles.css') === false && !/\.(tsx|ts|css)$/.test(f)) continue;
      if (/\.test\.|\.spec\./.test(f)) continue;
      const rel = relative(process.cwd(), f).replace(/\\/g, '/');
      readLines(f).forEach((line, i) => {
        for (const m of line.matchAll(VAR_REF_RE)) {
          const token = m[1].toLowerCase();
          if (!defs.has(token) && !ALLOWED_UNDEFINED[token]) {
            const list = missing.get(token) ?? [];
            list.push(`${rel}:${i + 1}`);
            missing.set(token, list);
          }
        }
      });
    }
    const msg = [...missing.entries()]
      .map(([token, locs]) => `${token} ← ${locs.join(', ')}（引用了未定义的令牌，浏览器里会静默失效）`)
      .join('\n');
    expect(msg).toBe('');
  });

  it('白名单令牌必须仍然被引用（防「引用删了白名单空挂」）', () => {
    const referenced = new Set<string>();
    for (const f of walkFiles(SRC_ROOT)) {
      if (f.endsWith('styles.css') === false && !/\.(tsx|ts|css)$/.test(f)) continue;
      if (/\.test\.|\.spec\./.test(f)) continue;
      for (const line of readLines(f)) {
        for (const m of line.matchAll(VAR_REF_RE)) referenced.add(m[1].toLowerCase());
      }
    }
    const stale = Object.keys(ALLOWED_UNDEFINED).filter((t) => !referenced.has(t));
    expect(stale).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// G3 令牌值唯一性
// ---------------------------------------------------------------------------
// 显式 alias 豁免：若两个令牌确实同值且语义上是别名，把名字组登记在这里并写明原因。
// 空 = 当前不存在任何 alias；将来加 alias 必须在这里登记，否则 G3 红。
const ALIAS_GROUPS: string[][] = [];

describe('G3 · 令牌值唯一性（UITEST-137）', () => {
  it('styles.css 不存在值相同的未登记 alias 令牌', () => {
    const css = readFileSync(join(SRC_ROOT, 'styles.css'), 'utf8');
    const byValue = new Map<string, string[]>();
    for (const m of css.matchAll(/(--[A-Za-z0-9-]+)\s*:\s*([^;]+);/g)) {
      const token = m[1].toLowerCase();
      const value = m[2].replace(/\s+/g, ' ').trim().toLowerCase();
      const list = byValue.get(value) ?? [];
      list.push(token);
      byValue.set(value, list);
    }
    const dupes = [...byValue.entries()].filter(([, tokens]) => tokens.length > 1);
    const offenders = dupes.filter(([, tokens]) =>
      !ALIAS_GROUPS.some((group) => tokens.every((t) => group.includes(t)))
    );
    const msg = offenders
      .map(([value, tokens]) => `${value} → ${tokens.join(', ')}（同值未登记 alias）`)
      .join('\n');
    expect(msg).toBe('');
  });
});

// ---------------------------------------------------------------------------
// 行为测试示范（§四）：user-event 真实交互 + aria/原生可访问性状态断言。
// 与既有 HotkeySettings.test.tsx 的区别：那边是 fireEvent + class 选择器；
// 这里用 user-event 走真实事件流，断言 checked/role 这类**编码设计意图**的属性。
// ---------------------------------------------------------------------------
const baseConfig = {
  auto_start: false,
  hotkey: { mode: 'toggle', vk_code: 0x78, modifiers: 0 },
  ui_language: 'Chinese',
  translation: { enabled: false, vk_code: 0, display_name: '', target_language: 'Chinese' },
};

function renderPage(config = baseConfig) {
  const updateConfig = vi.fn();
  // 本文件是 .ts（任务书指定产出文件名），oxc 不对 .ts 启用 JSX 语法，
  // 用 createElement 等价构造，语义与 JSX 完全一致。
  render(
    React.createElement(HotkeySettingsPage as React.FC<React.ComponentProps<typeof HotkeySettingsPage>>, {
      config,
      updateConfig,
    })
  );
  return { updateConfig };
}

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    setMaximizable: vi.fn().mockResolvedValue(undefined),
    metadata: {},
  }),
}));
vi.mock('@tauri-apps/api/app', () => ({
  getVersion: vi.fn().mockResolvedValue('0.5.3'),
}));

describe('行为示范 · user-event + aria（UITEST-137 §四）', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'check_hotkey_available') return true;
      return null;
    });
  });

  it('B1: 点 PushToTalk 单选 → 原生 checked 翻转 + updateConfig 携带新 mode', async () => {
    const user = userEvent.setup();
    const { updateConfig } = renderPage();
    const radios = screen.getAllByRole('radio');
    expect(radios).toHaveLength(2);
    const ptt = screen.getByRole('radio', { name: new RegExp(zhHans.hotkey_ptt) });
    // 初始：PushToTalk 未选中（断言原生 checked，不断言 class）
    expect(ptt).not.toBeChecked();
    await user.click(ptt);
    // onChange → updateConfig；aria 语义 = checked 属性，改名不影响
    expect(updateConfig).toHaveBeenCalledWith(
      expect.objectContaining({ hotkey: expect.objectContaining({ mode: 'PushToTalk' }) })
    );
  });

  it('B2: 录制中按 Escape → 回到热键按钮，updateConfig 不被调用', async () => {
    const user = userEvent.setup();
    const { updateConfig } = renderPage();
    // 语音热键按钮：accessible name = 当前热键显示名（0x78 = F9）
    await user.click(screen.getByRole('button', { name: 'F9' }));
    expect(screen.queryByRole('button', { name: 'F9' })).not.toBeInTheDocument();
    await user.keyboard('{Escape}');
    // 状态转换回按钮态；Escape 是取消，不产生配置写入
    expect(screen.getByRole('button', { name: 'F9' })).toBeInTheDocument();
    expect(updateConfig).not.toHaveBeenCalled();
  });

  it('B3: 翻译热键与语音热键撞键 → role="dialog" 的重复弹窗出现', async () => {
    const user = userEvent.setup();
    // 语音热键改用 A(0x41)：happy-dom 的 KeyboardEvent 不映射 F 键的 code
    // （实测 '{F9}' 交付 code='Unknown'，字母/Enter/Backspace 映射正常），
    // 依赖 e.code 的按键流在 user-event 下只能用字母键。
    const config = {
      ...baseConfig,
      hotkey: { ...baseConfig.hotkey, vk_code: 0x41 },
    };
    const updateConfig = vi.fn();
    render(
      React.createElement(HotkeySettingsPage as React.FC<React.ComponentProps<typeof HotkeySettingsPage>>, {
        config,
        updateConfig,
      })
    );
    // 切到翻译热键子页
    await user.click(screen.getByRole('button', { name: zhHans.hotkey_translation_tab }));
    // 开始录制翻译热键（显示名 = 未设置）
    await user.click(screen.getByRole('button', { name: zhHans.hotkey_not_set }));
    // 按下与语音热键相同的 A → 撞键
    await user.keyboard('a');
    // aria 断言：dialog 角色存在（modal 可访问性语义），并展示两侧显示名
    const dialog = screen.getByRole('dialog');
    expect(dialog).toBeInTheDocument();
    expect(dialog).toHaveTextContent(zhHans.hotkey_dup_title);
    expect(dialog).toHaveTextContent('A');
  });
});
