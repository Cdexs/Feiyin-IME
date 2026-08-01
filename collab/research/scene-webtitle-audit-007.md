# FIX-SCENE-WEBTITLE-007 · Web 关键词真实标题复核审计

> coder-1 2026-08-01。依据：Gavin 端测发现「飞书云文档」错配，主控要求全量复核 doc 35 条 + email 12 条 web 关键词的真实浏览器标题命中情况。
> 证据分级：✅实证（WebFetch 实抓 `<title>`）/ ⚠️部分确证（官网/登录页实抓，登录后文档页未抓，依已知稳定模式推断）/ ❌不命中（实证或强证据）。

---

## 〇、结论速览

| 项 | 结论 |
| --- | --- |
| 改动 1 `飞书云文档` | **已落地**（✅实证：真实文档页后缀即 `- 飞书云文档`，4 个独立公开 URL 一致）；`飞书文档` 保留 |
| 改动 2 `云文档` | **已落地**（规则性兜底，DEC-038）。实证：`金山文档 | WPS云文档 - KDocs` 真实标题出现 `WPS云文档`；`云文档`(3) > 钉钉/飞书(2) 最长匹配胜出 doc |
| 主要错配 | `滴答清单`（CN 站标题实际用 TickTick）、`Hotmail`（302→Outlook 永不含）、`Obsidian Publish`（真实标题无此串）、`Office Online`/`Online Doc`（不出现）、`Foxmail`/`Thunderbird`（无 web 版）、`邮件`/`发件箱`（中文站标题不用） |
| 危险等级 | **`钉钉文档`/`飞书云文档` 被 chat 短词截走 → 错判聊天**：已修复；其余不命中项大多落 browser(false) 保守降级，无 chat 截走风险 |

---

## 一、doc 块关键词复核（35 条）

| # | 关键词 | 真实标题格式 | 现关键词命中? | 若不命中：正确关键词 / 补充或替换 | 🔴 被谁抢走 |
|---|--------|------------|-------------|------------------------------|-----------|
| 1 | Google Docs | `{标题} - Google Docs`(EN) / `{标题} - Google 文档`(CN) | ✅EN 命中；⚠️CN UI 是 `Google 文档` 不命中 | 建议补充 `Google 文档`（CN 中文界面标题） | CN 界面落 browser(false) 保守降级，非 chat |
| 2 | 腾讯文档 | 官网 `腾讯文档-官方网站-...`；文档页推测 `{标题} - 腾讯文档` | ✅官网实证；文档页 ⚠️推测 | 无 | 无（无 chat 冲突） |
| 3 | 石墨文档 | 官网/登录页 `<title>石墨文档</title>` | ✅实证 | 无 | 无 |
| 4 | 飞书文档 | 真实文档页后缀 `- 飞书云文档`（✅实证）；docs.feishu.cn 官网 `飞书文档-可多人...` | ⚠️飞书文档命中官网；**飞书云文档 不命中**（原缺陷） | **`飞书云文档` 已补（改动1）**；飞书文档保留 | **原被 chat `飞书`(2) 截走 → 错判聊天（Gavin 实测）** |
| 5 | Notion | 官网 `... \| Notion`；文档页 `{标题} \| Notion` | ✅实证（官网+app 登录） | 无 | 无 |
| 6 | 语雀 | 首页 `语雀，... · Yuque`；文档页推测 `{标题} - 语雀` | ✅官网实证；文档页 ⚠️推测 | 无 | 无 |
| 7 | Jira | issue 页 `{KEY-123}: {摘要} - Jira` | ✅官网实证 + 已知模式 | 无 | 无 |
| 8 | TAPD | 官网 `TAPD-敏捷开发 项目管理...` | ✅官网实证；登录后页面 ⚠️ | 无 | 无 |
| 9 | 禅道 | 登录页 `<title>登录 - 禅道 - zentao.net</title>`（内部页 `{模块} - 禅道`） | ✅实证 | 无 | 无 |
| 10 | Teambition | 首页 `钉钉 Teambition · 阿里巴巴旗下团队协作工具` | ✅官网实证 | 无 | ⚠️首页含 `钉钉`，但 Teambition(9)>钉钉(2) 最长匹配胜出 doc，安全 |
| 11 | Google Keep | 标签页**恒定** `Google Keep` | ✅实证（keep.google.com 标题含） | 无 | 无 |
| 12 | 思源笔记 | Web 版标签默认 `SiYuan`(EN) / UI 中文时 `思源笔记` | ✅两者均已收（Gavin 端测实证） | 无 | 无 |
| 13 | SiYuan | 同 12 | ✅ | 无 | 无 |
| 14 | Obsidian Publish | **真实 `{页面标题} - {站点名}`**（如 `Home - Obsidian Help`）；**不含 "Obsidian Publish"** | ❌不命中（反证原假设） | ⚠️无法用固定关键词覆盖（站点名各异）；建议改匹配 `obsidian` 站点名或放弃 | 落 browser(false) 保守降级，非 chat |
| 15 | 金山文档 | `金山文档 \| WPS云文档 - KDocs`（✅实证） | ✅`金山文档` 命中 | `WPS 云文档` 为旧品牌仍出现 → **`云文档` 泛化已覆盖（改动2）** | 无（无 chat 冲突） |
| 16 | 钉钉文档 | 官网 `钉钉文档，提供安全、专业、实时...`（✅实证）；**非「钉钉云文档」** | ✅命中 | 无（品牌确认是 钉钉文档） | ⚠️标题含 `钉钉`，但 钉钉文档(4)>钉钉(2) 胜出 doc，安全 |
| 17 | Roam Research | 官网 `Roam Research – ...`；app 内标签 `Roam Research` | ✅实证 | 无 | 无 |
| 18 | Confluence | 真实 `{页面} \| {空间} \| Confluence`（Cloud） | ✅实证（confluence.atlassian.com 含 Confluence） | 无 | 无 |
| 19 | Anytype | 官网 `Anytype — ...`；app 内 `Anytype` | ✅官网实证；app 内 ⚠️ | 无 | 无 |
| 20 | HackMD | `{Note Title} - HackMD` | ✅实证 | 无 | 无 |
| 21 | StackEdit | `StackEdit`（app 恒定） | ✅实证 | 无 | 无 |
| 22 | Dillinger | `Markdown Editor — Online, Free... \| Dillinger` | ✅实证 | 无 | 无 |
| 23 | Trilium | `{Note Title} - Trilium Notes` | ✅`Trilium` 命中（子串） | 无（`TriliumNext` 不出现，勿用） | 无 |
| 24 | Standard Notes | `Notes · Standard Notes`（列表）/ `{Note} - Standard Notes` | ✅实证 | 无 | 无 |
| 25 | Todoist | `Todoist`（app 恒定） | ✅实证 | 无 | 无 |
| 26 | TickTick | `TickTick: A To-Do List...`（EN） | ✅实证 | 无 | 无 |
| 27 | 滴答清单 | **CN 站标题用 `TickTick`，实证 0 次出现「滴答清单」** | ❌不命中（web 标题） | ⚠️补 `TickTick` 已覆盖中英文站；`滴答清单` 保留无害（防桌面窗口标题命中——但 title_keywords 仅浏览器细分用） | 无（TickTick 兜住 → doc） |
| 28 | Trello | 看板 `{Board} \| Trello` | ✅实证（登录页含） | 无 | 无 |
| 29 | Asana | `{Task/Project} - Asana` | ✅实证（登录页含） | 无 | 无 |
| 30 | ClickUp | `ClickUp`（app 恒定） | ✅实证 | 无 | 无 |
| 31 | Google Tasks | 独立页 `Google Tasks`（tasks.google.com）；Gmail/Calendar 侧栏时标签为宿主标题 | ✅实证独立页 | ⚠️注意：侧栏嵌入时检测不到 | 无 |
| 32 | Microsoft To Do | **`Microsoft To Do`（空格无连字符）** | ✅命中（我们的关键词正是带空格形式） | ⚠️旧品牌曾有 `Microsoft To-Do` 连字符变体，可考虑补 | 无 |
| 33 | Any.do | `Sign in to Any.do`（登录）/ app 页含 `Any.do` | ✅实证 | 无 | 无 |
| 34 | Online Doc | ⚠️不出现于现行标题；文档页实际 `{标题} - Word Online`/`Excel Online`/`PowerPoint Online` | ❌不命中 | 建议替换为 `Word Online`/`Excel Online`/`PowerPoint Online`/`Microsoft 365` | 落 browser(false) 保守降级，非 chat |
| 35 | Office Online | 同 34，`office.com` 现跳 `m365.cloud.microsoft` | ❌不命中 | 同上 | 落 browser(false) 保守降级，非 chat |

---

## 二、email 块关键词复核（12 条）

| # | 关键词 | 真实标题格式 | 现关键词命中? | 若不命中：正确关键词 / 补充或替换 | 🔴 被谁抢走 |
|---|--------|------------|-------------|------------------------------|-----------|
| 1 | Outlook | 收件箱 `Inbox – user@outlook.com – Outlook` / 中文 `收件箱 – ... – Outlook` | ✅实证（登录页 `<title>Outlook</title>`） | 无 | 无 |
| 2 | Foxmail | **无 web 版**（仅 Windows/Mac 桌面客户端） | ❌不命中（浏览器永不含） | ⚠️无 web 版，保留无害但不生效 | 无（不出现） |
| 3 | Thunderbird | **无 web 版**；web 产品为 `Thundermail`（tb.pro） | ❌不命中 | ⚠️若想兜 Thundermail 可加（本批不动） | 无（不出现） |
| 4 | 邮件 | 实证标题无（中文站用 收件箱/邮箱） | ❌不命中 | ⚠️建议剔除或保留无害 | 无 |
| 5 | 邮箱 | `QQ邮箱` 登录标题含；163/126 `<title>` 不含 | ⚠️部分命中（QQ）；泛词误报风险 | 保留（QQ邮箱命中） | 无 |
| 6 | Mail | Yahoo/Proton/Mail.ru 均含；**泛词**（Daily Mail/mailto 误报） | ⚠️命中但误报风险 | 保留（兜 Yahoo/Proton 等） | 无 |
| 7 | Gmail | 登录 `Gmail`；收件箱 `Inbox (12) – user@gmail.com – Gmail` | ✅实证 | 无 | 无 |
| 8 | Inbox | Outlook/Gmail/Yahoo/Zoho 英文收件箱均含；泛词 | ⚠️命中但泛词（Slack 等后台也含） | 保留（英文收件箱真实命中） | 无 |
| 9 | 收件箱 | 中文 webmail 收件箱标签真实出现（163/126/QQ/Gmail中文） | ✅实证 | 无 | 无 |
| 10 | 发件箱 | 中文站发件文件夹标题用 `已发送` 非 `发件箱` | ❌不命中 | ⚠️建议替换 `已发送` 或保留无害 | 无 |
| 11 | Yahoo Mail | 首页 `Yahoo Mail | ...`；收件箱 `Inbox (n) – Yahoo Mail` | ✅实证 | 无 | 无 |
| 12 | Hotmail | **hotmail.com 302→Outlook，标题恒为 `Outlook`，永不含 Hotmail** | ❌不命中（反证：老用户标题也是 Outlook） | ⚠️Hotmail 关键字无效，但保留无害（Outlook 已兜住）；不必收 | 无（Outlook 兜住 → email） |

---

## 三、本批落地项

| 项 | 落地 |
| --- | --- |
| `飞书云文档` | ✅已加 doc title_keywords（改动 1，Gavin 端测实证） |
| `云文档` | ✅已加 doc title_keywords（改动 2，规则性兜底；实证 `WPS云文档` 真实标题存在） |

其余不命中项（Obsidian Publish / 滴答清单 / Hotmail / Office Online / Online Doc / 邮件 / 发件箱 / Google 文档 CN / Thundermail 等）**只列不改**，等待主控逐条裁定。

---

## 四、危险等级说明（「被谁抢走」列的判据）

- **🔴 chat 截走（最高危）**：仅 `钉钉`/`飞书` 两族（chat 短词 + 长词不命中时会落 chat/false，不给多行）。`钉钉文档` 已确认品牌正确（不会被截）；`飞书云文档` 已补。其余 doc 关键词对应标题均不含 chat 短词，**无此风险**。
- **🟡 browser(false) 保守降级（无害）**：Obsidian Publish / Office Online / Online Doc / Google 文档(CN) 等不命中时落 browser，`multiline_safe=false` 保守处理，等同 Phase 1，不会错判但也不给多行。
- **🟢 无影响**：Foxmail/Thunderbird（无 web 版不出现）、Hotmail（Outlook 兜住）、滴答清单（TickTick 兜住）。

## 五、建议后续（非本批）

1. 若端测发现其他「云文档」类变体（华为云文档/腾讯云文档），`云文档` 已规则性兜住，无需逐个加。
2. `Google 文档`（CN 界面）可作为 doc 关键词补充（本批未动）。
3. `Word Online`/`Excel Online`/`PowerPoint Online` 可替换失效的 `Office Online`/`Online Doc`（本批未动）。
4. `已发送` 可替换失效的 `发件箱`（本批未动）。
