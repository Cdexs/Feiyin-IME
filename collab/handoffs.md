# handoffs · voice-ime

> 只保留当天条目；历史条目见 `handoffs-archive.md`。



## 2026-09-08 — coder-2 — EDITICON-190 ✅ A2 几何迁入生产（Gavin 定裁「图标选a2」，待主控验收 → 与 188/189 一批 commit）

- **改动**：`menu_icons.rs` 只换常量——`PENCIL_TIP→(-0.24,0.26)`、`PENCIL_END→(0.26,-0.24)`、`UNDERLINE→(0.361,-0.42,0.40)`（HALF_W/S_TIP/S_GAP_END 不变）；三个 covered 函数体逐字节未动；三 pub 签名不变 → main.rs D2D/GDI 自动吃新像素，免集成单。
- **🔴 参数留底**：A1 全套（`A1_TIP/A1_END/A1_LINE` + `a1_pencil_covered/a1_covered` + `dump_edit_icon_a1_preview`）留 cfg(test) 对照组，注释标明「勿删」；完整常量表见 outbox/coder-2/result.md。
- **逐字节自证**：生产重 dump 的 `edit-A2-18.png` 与 Gavin 定裁预览 **md5 一致 `187ab98e3311d791ee94acab04f49cd5`**。
- **验证**：fmt 幂等 / check 零 error、warnings **111/102** 持平 / 全量 **1113P/0F**（-a2 测试 +a1 对照测试，用例数不变）/ 零字体 grep=0 / diff 仅 menu_icons.rs（main.rs 98+/36- 为 coder-1 STREAMFONT-189 在途，零触碰）。
- **预览**：icons/ 16 张全保留（edit-A-* / edit-A1-* / edit-A2-* / edit-FINAL-* 各 {18,16}+x8）。
- **红线**：未 commit/版本未动/未出包（Gavin 攒批）/零凭证。



## 2026-09-08 — coder-2 — EDITICON-187-ALT ✅ A2 变体预览（cfg(test)，生产零改动，待 Gavin 对 A1/A2 二选一）

- **A2 几何**：铅笔整体下移——tip y=0.26（推导自档案气隙 1.3px）、tip x=-0.24 / END=(0.26,-0.24)（纯推算：沿 175→187 迁移线插值，result.md 已分列「档案实数/推导/推算」）；下划线 y=0.361、x -0.42..0.40 档案原值逐位照抄。
- **改动**：全在 `mod tests`（A2 常量 + `a2_pencil_covered` 参数化副本 + `a2_covered` + `dump_edit_icon_a2_preview`，+~110 行）；生产 `edit_icon_covered/UNDERLINE/pencil_covered/PENCIL_*` 逐字节未动。
- **A1 vs A2**（18px）：A1 笔尖行 11.5/线行 13/气隙 1.5px/线下留白 4 行、铅笔盒缘裁切原样；A2 笔尖行 13.7/线行 15/气隙 1.3px（档案）/线下留白 2 行、铅笔 s=1.0 角内收尾（右上 ~0.8px 空隙）。
- **验证**：fmt 幂等 / check 零 error、warnings **111/102** 持平 / 全量 **1113P/0F**（+1=A2 测试）/ 零字体 grep=0 / diff 仅 menu_icons.rs（main.rs +7/-0 为 coder-1 ESC-188 在途）。
- **预览**：`outbox/coder-2/icons/` 12 张 = `edit-A-*`(A1) + `edit-A2-*`(A2) + `edit-FINAL-*`(B) 各 {18,16}+x8，全保留供 Gavin 并排比。
- **红线**：未 commit/版本未动/未出包（Gavin 攒批）/零凭证。



## 2026-09-08 — coder-2 — EDITICON-187 ✅ 编辑图标换回候选 A「铅笔+单条下划线」（铅笔逐位不动，待主控验收 → Gavin 目视 → 并入下次 BUILD）

- **取证重建**：A 常量未留底（185 清理 + 184/185 squash），档案 `logs/20260907.md:571` 命中 A 下划线参数（y=0.361、x -0.42..0.40、7px/1.8px 邻距、笔尖-线 1.3px 气隙）。
- **关键取舍**：184 的 A 铅笔是「上移缩距」版（笔位与 185 定稿不同，原始常量丢失）；任务书红线「铅笔逐位不动」优先 → 下划线改 y=0.25 补偿，笔尖-基线气隙 1.5px ≈1.3px 构图；x/-0.42..0.40 按 archive（7px 净距/1.8px 盒缘）。
- **改动**：`menu_icons.rs` 44+/32-（UNDERLINE 单线 + `:27` 注释订正 + 测试断言换 A）；铅笔五常量逐位未动；main.rs 零触碰（+7/-0 为 coder-1 ESC-188 在途）。
- **验证**：fmt 幂等 / check 零 error、warnings **111/102** 持平 / 全量 **1112P/0F** / 零字体 grep=0 / diff 仅 menu_icons.rs 一文件。
- **预览**：`outbox/coder-2/icons/edit-A-{18,16}+x8` 4 张，**保留不清理**（任务书明示）。待主控目视 → 转 Gavin 确认形态 → 通过则并入下次 BUILD（本单零 main.rs 改动，D2D/GDI 兜底自动吃新像素，免集成单）。
- **红线**：未 commit/版本未动/零凭证/未 cargo build --release。



## 2026-09-08 — tester-1 — BUILD-186 ✅ 出包：ESC-178 + EDITFONT-183 + EDITICON-185 定稿（精简流程，跨日构建，待主控验收）

- **基线**：HEAD `fd527e0` clean（ESC-178/EDITFONT-183/EDITICON-179~185 全链已由主控提交）。构建 00:48-00:50 跨日，沿用 20260907.md logs。
- **精简流程**：Step1 清进程（无残留）→ Step2 git-log 法 UI 免重建（f85c550@09-06 18:47 早于 UI exe 01:15）→ Step3 主程序（2m10s，feiyin-ime 111 / crash-reporter 5）→ Step4 cp -p 同步 Publish/。
- **七项核验全 PASS**：① 主程序 09-08 00:50 本次构建；② 两副本 sha `6624cdd1…` 一致异于 `9d60f458…`；③ ProductVersion 0.9.0.0；④ 冒烟 PID 25412 Responding=True 无 panic 已清理；⑤ config.toml sha `3186ec8c` 不变；⑥ warnings 111/102 持平；⑦ 🔴 **二进制字面量判别探针正反对照 PASS**（`ESC-178:` debug 日志可构造：PRE186 反向 0 命中 / 新包正向 1 命中含 7 个前缀片段；`OVERLAY_EDIT_FONT_SIZE` 源码 grep=6 ≥5）。
- **回归（并行）**：cargo test 全量 **1112P/0F/9I** + hotkey 51P + ui_guard 2P；Vitest/E2E Skip。🔴 crash_reporter config 批量 FAIL **本轮未出现**（已立跟踪项）；asr_074/asr_056 异域偶发也未出现。
- **出包语义**：ESC-178 轮询旁路（🔴 原消息路由机制未定待 debug 日志端测定案）+ EDITFONT-183 编辑框 14→16px + EDITICON-185 图标定稿候选 B（铅笔+两条短文本线，Gavin 选定）；🔴 已知取舍三条如实写入（选区反白不渲染/他窗 ESC 也取消/文字 14→16 跳变）。不出端测清单给 Gavin，主控自出。
- **红线**：未 commit / v0.9.0 未动 / 未 cargo clean / 未 cargo tauri build / 零凭证 / 无临时文件。



