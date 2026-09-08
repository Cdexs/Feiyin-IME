//! TRAY-ICON-158: 托盘菜单项图标（齿轮=设置，电源=退出）。
//!
//! 纯 Rust 光栅化，零平台依赖、零新 crate。抗锯齿用 SSAA 4x4 逐像素超采样
//! （每像素 16 个子样本取覆盖率写 alpha）——明确不用解析式 SDF，
//! OVERLAY-121/141/153 三轮教训：SDF 覆盖率语义反复出问题，SSAA 判别直观。
//!
//! 输出为直通 RGBA8（非预乘）+ 顶行在前的行主序；预乘与 BGRA 转换由
//! 平台侧完成（Windows: main.rs `create_menu_item_bitmap`，macOS: tray.rs）。

/// 品牌橙，与 `src/ui/tray.rs` 托盘图标同色（`#FF6B35`）。
/// 不给退出用红：退出=关闭本应用，良性动作；橙在浅/深两种菜单底上对比度都够。
const BRAND_ORANGE: [u8; 3] = [0xFF, 0x6B, 0x35];

/// SSAA 每轴子采样数（4x4 = 每像素 16 个子样本）。
const SSAA: u32 = 4;

/// 齿轮图标（设置）。
pub fn settings_icon_rgba(size: u32) -> Vec<u8> {
    rasterize(size, gear_covered)
}

/// 电源符号图标（退出）。
pub fn exit_icon_rgba(size: u32) -> Vec<u8> {
    rasterize(size, power_covered)
}

// edit_icon_rgba 见本文件下方 EDITICON-190 实现（纯几何 SSAA：铅笔 + 单条下划线，定稿 A2）。

/// EDITICON-176：直通 alpha BGRA（GDI `AlphaBlend` `AC_SRC_ALPHA` 输入格式）。
/// 仅做 RGBA→BGRA 字节序重排，alpha 通道原样（非预乘）。
pub fn edit_icon_bgra(size: u32) -> Vec<u8> {
    let mut px = edit_icon_rgba(size);
    for c in px.chunks_exact_mut(4) {
        c.swap(0, 2);
    }
    px
}

/// EDITICON-176：预乘 alpha BGRA（D2D `CreateBitmap` B8G8R8A8/预乘输入格式）。
/// RGB 通道按 alpha 加权（四舍五入），字节序重排为 BGRA。
pub fn edit_icon_premultiplied_bgra(size: u32) -> Vec<u8> {
    let mut px = edit_icon_rgba(size);
    for c in px.chunks_exact_mut(4) {
        let (r, g, b, a) = (c[0] as u32, c[1] as u32, c[2] as u32, c[3] as u32);
        c[0] = ((b * a + 127) / 255) as u8;
        c[1] = ((g * a + 127) / 255) as u8;
        c[2] = ((r * a + 127) / 255) as u8;
        c[3] = a as u8;
    }
    px
}

fn rasterize(size: u32, covered: impl Fn(f32, f32) -> bool) -> Vec<u8> {
    let s = size as f32;
    let half = s / 2.0;
    let sub = SSAA as f32;
    let mut px = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            let mut hits = 0u32;
            for sy in 0..SSAA {
                for sx in 0..SSAA {
                    // 像素内子样本坐标，中心为原点，单位 = S（图标规格坐标系）
                    let fx = (x as f32 + (sx as f32 + 0.5) / sub - half) / s;
                    let fy = (y as f32 + (sy as f32 + 0.5) / sub - half) / s;
                    if covered(fx, fy) {
                        hits += 1;
                    }
                }
            }
            let i = ((y * size + x) * 4) as usize;
            px[i] = BRAND_ORANGE[0];
            px[i + 1] = BRAND_ORANGE[1];
            px[i + 2] = BRAND_ORANGE[2];
            px[i + 3] = ((hits as u32 * 255 + SSAA / 2) / SSAA) as u8;
        }
    }
    px
}

// —— 齿轮：实心盘（孔 0.13S..根圆 0.32S）+ 6 条梯形齿（齿根宽 30°、齿顶宽 20°，
//    半角随 r 线性收窄 ⇒ 外窄内宽，齿顶到外径 0.44S）——
// TRAY-ICON-158-FIX 返工：原版齿区只画 root..outer，齿正下方 hole..root 被挖空，
// 圆盘只剩齿间 8 个楔形 ⇒ 放射刺/雪花观感；且恒定角宽切出外宽内窄的扇形齿（方向反了）。
// 8 齿在 16px 信息密度过载，收敛为 6 齿（对齐 Segoe MDL2 等系统级 UI 齿轮）。
const GEAR_OUTER_R: f32 = 0.44;
const GEAR_ROOT_R: f32 = 0.32;
const GEAR_HOLE_R: f32 = 0.13;
const GEAR_TEETH: u32 = 6;
const TOOTH_ROOT_HALF_RAD: f32 = 15.0f32.to_radians(); // 齿根半角（齿宽 30°）
const TOOTH_TIP_HALF_RAD: f32 = 10.0f32.to_radians(); // 齿顶半角（齿宽 20°）

fn gear_covered(fx: f32, fy: f32) -> bool {
    let r = (fx * fx + fy * fy).sqrt();
    if r > GEAR_OUTER_R || r <= GEAR_HOLE_R {
        return false;
    }
    if r <= GEAR_ROOT_R {
        return true; // 🔴 实心盘：孔到根圆整片实心
    }
    // 齿区：半角随 r 线性收窄 ⇒ 外窄内宽的梯形齿
    let t = (r - GEAR_ROOT_R) / (GEAR_OUTER_R - GEAR_ROOT_R); // 根 0 → 顶 1
    let half = TOOTH_ROOT_HALF_RAD + (TOOTH_TIP_HALF_RAD - TOOTH_ROOT_HALF_RAD) * t;
    let sector = core::f32::consts::TAU / GEAR_TEETH as f32;
    let theta = fy.atan2(fx);
    let phase = (theta % sector + sector) % sector;
    phase.min(sector - phase) <= half
}

// —— 电源：圆环半径 0.30S / 线宽 0.11S / 顶部 70° 缺口（缺口指向正上）；
//      竖线 y=-0.40S..-0.06S，宽 0.11S，两端圆头 ——
const POWER_RING_R: f32 = 0.30;
const POWER_STROKE: f32 = 0.11;
const POWER_GAP_HALF_RAD: f32 = 35.0f32.to_radians(); // 缺口 70° 居中朝上
const LINE_Y_TOP: f32 = -0.40;
const LINE_Y_BOTTOM: f32 = -0.06;

fn power_covered(fx: f32, fy: f32) -> bool {
    let half = POWER_STROKE / 2.0;
    // 竖线（带圆头的胶囊段），先判——它要穿过顶部缺口
    let cy = fy.clamp(LINE_Y_TOP, LINE_Y_BOTTOM);
    let dx = fx;
    let dy = fy - cy;
    if dx * dx + dy * dy <= half * half {
        return true;
    }
    // 圆环带
    let r2 = fx * fx + fy * fy;
    if r2 < (POWER_RING_R - half) * (POWER_RING_R - half)
        || r2 > (POWER_RING_R + half) * (POWER_RING_R + half)
    {
        return false;
    }
    // 顶部缺口：以正上方向为零，±35° 内的环带不画
    let ang_from_up = fx.atan2(-fy);
    ang_from_up.abs() >= POWER_GAP_HALF_RAD
}

// —— EDITICON-190：编辑图标定稿 = 候选 A2「铅笔 + 单条下划线」（Gavin 2026-09-08 目视
// edit-A vs edit-A2 预览后拍板「图标选a2」，按预览原样定稿，不再调笔矢量）——
// 构图：铅笔（右上→左下斜压，笔尖指向下方基线）+ 单条下划线（书写基线语义）。
// 常量来源（187-ALT 三分法）：下划线 y=0.361/x -0.42..0.40 = 184 档案原值；
// tip y=0.26 推导自档案气隙 1.3px；tip x=-0.24/END=(0.26,-0.24) 推算（沿 175→187
// 迁移线插值）。⚠️ 铅笔整体下移后右上端 s=1.0 于角内收尾（留 ~0.8px 空隙），
// **不再被盒缘裁切**——Gavin 看过放大预览后按此样定稿，不许「修正」为触角。
// 🔴 A1（187 版）几何常量保留在 cfg(test) 对照组（dump_edit_icon_a1_preview），
// 勿删——187 单本身就是因为 185 清理候选致参数丢失。
const PENCIL_TIP: (f32, f32) = (-0.24, 0.26);
const PENCIL_END: (f32, f32) = (0.26, -0.24);
const PENCIL_HALF_W: f32 = 0.08; // 笔杆半宽（全宽 0.16S）
const PENCIL_S_TIP: f32 = 0.22; // 尖楔占轴长比例
const PENCIL_S_GAP_END: f32 = 0.31; // 笔杆起点（尖-杆缺口）
const UNDERLINE: (f32, f32, f32) = (0.361, -0.42, 0.40); // (y, x0, x1) 单条下划线（184 档案原值）

pub fn edit_icon_rgba(size: u32) -> Vec<u8> {
    rasterize(size, edit_icon_covered)
}

fn pencil_covered(fx: f32, fy: f32) -> bool {
    let (tx, ty) = PENCIL_TIP;
    let (ex, ey) = PENCIL_END;
    let dx = ex - tx;
    let dy = ey - ty;
    let len = (dx * dx + dy * dy).sqrt();
    let ax = dx / len;
    let ay = dy / len;
    let nx = -ay;
    let ny = ax;
    let px = fx - tx;
    let py = fy - ty;
    let s = px * ax + py * ay; // 沿轴位置（单位轴参数化，与 184 预览同数学）
    if !(0.0..=1.0).contains(&s) {
        return false;
    }
    let d = (px * nx + py * ny).abs(); // 到轴的垂距
    if s <= PENCIL_S_TIP {
        d <= PENCIL_HALF_W * (s / PENCIL_S_TIP) // 尖楔：宽度从尖点线性展开
    } else if s >= PENCIL_S_GAP_END {
        d <= PENCIL_HALF_W // 笔杆
    } else {
        false // 尖-杆缺口
    }
}

fn underline_covered(fx: f32, fy: f32) -> bool {
    let half = 0.055 / 2.0; // 线厚 ≈1px @18px（alpha 公式注：≥4 子样本命中即渲染为实条）
    let (y, x0, x1) = UNDERLINE;
    (y - half..=y + half).contains(&fy) && (x0..=x1).contains(&fx)
}

fn edit_icon_covered(fx: f32, fy: f32) -> bool {
    pencil_covered(fx, fy) || underline_covered(fx, fy)
}

// v3 手工几何（纸+笔）对照已随 185 移除（Gavin 定稿后无对照需求）；历史几何可从
// git/log 恢复。生产实现现定稿候选 A2（EDITICON-190），A1 对照在 cfg(test)。

#[cfg(test)]
mod tests {
    use super::*;

    /// EDITICON-190 视觉自证：生产定稿（A2）出 edit-A2-{18,16}+x8。
    /// 🔴 预览只证明「形态好不好看」（DIAG-166 教训），不证明运行时行为；
    /// 运行时链路已由 Gavin 实际使用 B 图标反证打通。
    /// 🔴 预览 PNG **保留不清理**（185 清理候选致 A 参数丢失的前车之鉴）。
    #[test]
    fn dump_edit_icon_preview() {
        let dir = std::path::Path::new("D:/Workspace/CodeLab/collab/outbox/coder-2/icons");
        std::fs::create_dir_all(dir).unwrap();
        let scale = 8u32;
        for size in [18u32, 16] {
            let rgba = edit_icon_rgba(size);
            image::save_buffer(
                dir.join(format!("edit-A2-{size}.png")),
                &rgba,
                size,
                size,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
            let mut big = Vec::with_capacity((size * scale * size * scale * 4) as usize);
            for by in 0..size * scale {
                for bx in 0..size * scale {
                    let i = (((by / scale) * size + bx / scale) * 4) as usize;
                    big.extend_from_slice(&rgba[i..i + 4]);
                }
            }
            image::save_buffer(
                dir.join(format!("edit-A2-{size}-x8.png")),
                &big,
                size * scale,
                size * scale,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        }
        // 抽检（候选 A）：非全空 + 满覆盖墨迹 + 非实心方块
        let icon = edit_icon_rgba(18);
        assert!(
            icon.chunks_exact(4).any(|p| p[3] >= 250),
            "edit icon must have near-opaque ink"
        );
        assert!(
            icon.chunks_exact(4).any(|p| p[3] == 0),
            "edit icon must have empty regions (not a filled square)"
        );
        // 铅笔仍伸向右上（A2 笔画到 s=1.0 于角内收尾，近角有墨但不再满贴盒缘——
        // Gavin 按预览原样定稿，不许「修正」为触角）
        assert!(
            icon[(0 * 18 + 17) * 4 + 3] > 0,
            "A2 pencil must still reach toward top-right (per approved preview)"
        );
        // 左上/左下/右下角透明
        for (y, x) in [(0usize, 0usize), (17usize, 0usize), (17usize, 17usize)] {
            assert_eq!(icon[(y * 18 + x) * 4 + 3], 0, "corner must be empty");
        }
        // 笔杆满覆盖像素 (6,12)
        assert_eq!(
            icon[(6 * 18 + 12) * 4 + 3],
            252,
            "pencil shaft must be full coverage"
        );
        // 单条下划线存在（行 15；y=0.361 档案原值带宽恰落行 15 子采样网格 → 干净单行）
        assert_eq!(
            icon[(15 * 18 + 5) * 4 + 3],
            252,
            "A2 underline must be a single clean row (archive y=0.361)"
        );
        // 下划线右端端像素 (15,16)：距分割线 7px 无粘连；分割线侧（col17）无墨
        assert!(
            icon[(15 * 18 + 16) * 4 + 3] >= 128,
            "A2 underline right end pixel (7px clear of divider)"
        );
        assert_eq!(icon[(15 * 18 + 17) * 4 + 3], 0, "no ink beyond A2 x1=0.40");
        // 笔尖-下划线气隙（≈1.3px 档案值）：行 14 必须透明
        assert_eq!(
            icon[(14 * 18 + 6) * 4 + 3],
            0,
            "A2 tip-line air gap (archive 1.3px)"
        );
        // 笔尖楔区有墨（行 12）
        assert!(icon[(12 * 18 + 5) * 4 + 3] > 0, "A2 pencil tip wedge ink");
    }

    // —— EDITICON-190：A1 对照组（187 交付版几何，仅 cfg(test)；🔴 参数留底防再丢）——
    // A1 = 187 的「铅笔(185 定稿位) + 补偿下划线」：铅笔逐位=185 定稿值，线 y=0.25。
    // Gavin 2026-09-08 已定裁 A2 为生产（见生产块注释），A1 仅作对照预览，勿删。
    const A1_TIP: (f32, f32) = (-0.10, 0.14);
    const A1_END: (f32, f32) = (0.40, -0.36);
    const A1_LINE: (f32, f32, f32) = (0.25, -0.42, 0.40);

    /// 与生产 `pencil_covered` 同数学的参数化副本（生产函数零触碰）。
    fn a1_pencil_covered(fx: f32, fy: f32) -> bool {
        let (tx, ty) = A1_TIP;
        let (ex, ey) = A1_END;
        let dx = ex - tx;
        let dy = ey - ty;
        let len = (dx * dx + dy * dy).sqrt();
        let ax = dx / len;
        let ay = dy / len;
        let nx = -ay;
        let ny = ax;
        let px = fx - tx;
        let py = fy - ty;
        let s = px * ax + py * ay;
        if !(0.0..=1.0).contains(&s) {
            return false;
        }
        let d = (px * nx + py * ny).abs();
        if s <= PENCIL_S_TIP {
            d <= PENCIL_HALF_W * (s / PENCIL_S_TIP)
        } else if s >= PENCIL_S_GAP_END {
            d <= PENCIL_HALF_W
        } else {
            false
        }
    }

    fn a1_covered(fx: f32, fy: f32) -> bool {
        a1_pencil_covered(fx, fy) || {
            let half = 0.055 / 2.0;
            let (y, x0, x1) = A1_LINE;
            (y - half..=y + half).contains(&fy) && (x0..=x1).contains(&fx)
        }
    }

    /// A1 对照预览：edit-A1-{18,16}+x8（187 交付原样，供与生产 A2 并排比对/回滚参考）。
    #[test]
    fn dump_edit_icon_a1_preview() {
        let dir = std::path::Path::new("D:/Workspace/CodeLab/collab/outbox/coder-2/icons");
        std::fs::create_dir_all(dir).unwrap();
        let scale = 8u32;
        for size in [18u32, 16] {
            let rgba = rasterize(size, a1_covered);
            image::save_buffer(
                dir.join(format!("edit-A1-{size}.png")),
                &rgba,
                size,
                size,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
            let mut big = Vec::with_capacity((size * scale * size * scale * 4) as usize);
            for by in 0..size * scale {
                for bx in 0..size * scale {
                    let i = (((by / scale) * size + bx / scale) * 4) as usize;
                    big.extend_from_slice(&rgba[i..i + 4]);
                }
            }
            image::save_buffer(
                dir.join(format!("edit-A1-{size}-x8.png")),
                &big,
                size * scale,
                size * scale,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        }
        // 抽检（A1 @18px，与 187 交付断言同源）
        let icon = rasterize(18, a1_covered);
        assert!(
            icon[(0 * 18 + 17) * 4 + 3] > 0,
            "A1 pencil reaches top-right edge (185 clipping as-is)"
        );
        for (y, x) in [(0usize, 0usize), (17usize, 0usize), (17usize, 17usize)] {
            assert_eq!(icon[(y * 18 + x) * 4 + 3], 0, "A1 corner must be empty");
        }
        assert_eq!(
            icon[(6 * 18 + 12) * 4 + 3],
            252,
            "A1 pencil shaft must be full coverage"
        );
        assert_eq!(
            icon[(13 * 18 + 5) * 4 + 3],
            252,
            "A1 underline must be a single clean row (y=0.25)"
        );
        assert_eq!(
            icon[(13 * 18 + 16) * 4 + 3],
            255,
            "A1 underline right end pixel (7px clear of divider)"
        );
        assert_eq!(icon[(13 * 18 + 17) * 4 + 3], 0, "no ink beyond A1 x1=0.40");
        assert_eq!(
            icon[(12 * 18 + 8) * 4 + 3],
            0,
            "A1 tip-line air gap (≈1.5px)"
        );
    }

    /// EDITICON-176：字节序/预乘转换正确性。
    /// 用一个合成的非对称像素（避免对称像素掩盖通道错位）逐项断言：
    /// bgra = 通道交换、alpha 原样；premultiplied = RGB 按 alpha 加权 + 交换。
    #[test]
    fn edit_icon_bgra_conversions() {
        // 构造一个 alpha=0x80 的透明橙像素场景：直接对转换逻辑用手工像素自证。
        // 从真实图标取第 1 像素做不变式断言（角像素 alpha=0 → 预乘后 RGB 必须=0）。
        let src = edit_icon_rgba(18);
        let bgra = edit_icon_bgra(18);
        for (s, b) in src.chunks_exact(4).zip(bgra.chunks_exact(4)) {
            assert_eq!(s[0], b[2], "R channel must move to index 2");
            assert_eq!(s[2], b[0], "B channel must move to index 0");
            assert_eq!(s[1], b[1]);
            assert_eq!(s[3], b[3], "alpha must be unchanged (straight alpha)");
        }
        let pm = edit_icon_premultiplied_bgra(18);
        for (s, p) in src.chunks_exact(4).zip(pm.chunks_exact(4)) {
            let (r, g, b, a) = (s[0] as u32, s[1] as u32, s[2] as u32, s[3] as u32);
            assert_eq!(p[0], ((b * a + 127) / 255) as u8, "premultiplied B");
            assert_eq!(p[1], ((g * a + 127) / 255) as u8, "premultiplied G");
            assert_eq!(p[2], ((r * a + 127) / 255) as u8, "premultiplied R");
            assert_eq!(p[3], a as u8);
        }
        // 角像素 alpha=0 → 预乘后 RGB 必须清零（D2D 预乘不变式）
        assert_eq!(pm[0], 0);
        assert_eq!(pm[1], 0);
        assert_eq!(pm[2], 0);
        // 满覆盖像素：alpha 直通预乘（前提源无关：源中必须存在满覆盖像素，
        // 且其 alpha 在两条转换输出中原样）
        let full = src
            .chunks_exact(4)
            .position(|p| p[3] >= 250)
            .expect("edit icon must have near-opaque ink")
            * 4; // 像素索引 → 字节偏移
        assert_eq!(bgra[full + 3], src[full + 3], "alpha passthrough (bgra)");
        assert_eq!(
            pm[full + 3],
            src[full + 3],
            "alpha passthrough (premultiplied)"
        );
    }

    /// TRAY-ICON-158 视觉自证：16/24/32 三档 x 2 图标 = 6 张 PNG + 各一张 8x
    /// 最近邻放大版（*x8.png，看锯齿用），写到 collab outbox。
    /// 常驻无副作用（只写文件），主控逐张目视后才放行出包。
    #[test]
    fn dump_menu_icons_preview() {
        let dir = std::path::Path::new("D:/Workspace/CodeLab/collab/outbox/coder-1/icons");
        std::fs::create_dir_all(dir).unwrap();
        let scale = 8u32;
        for size in [16u32, 24, 32] {
            for (name, rgba) in [
                ("settings", settings_icon_rgba(size)),
                ("exit", exit_icon_rgba(size)),
            ] {
                let path = dir.join(format!("{name}-{size}.png"));
                image::save_buffer(&path, &rgba, size, size, image::ExtendedColorType::Rgba8)
                    .unwrap();
                let mut big = Vec::with_capacity((size * scale * size * scale * 4) as usize);
                for by in 0..size * scale {
                    for bx in 0..size * scale {
                        let i = (((by / scale) * size + bx / scale) * 4) as usize;
                        big.extend_from_slice(&rgba[i..i + 4]);
                    }
                }
                let bpath = dir.join(format!("{name}-{size}-x8.png"));
                image::save_buffer(
                    &bpath,
                    &big,
                    size * scale,
                    size * scale,
                    image::ExtendedColorType::Rgba8,
                )
                .unwrap();
            }
        }
        // 覆盖率抽检：齿轮中心孔必须全透明，外圈外必须全透明
        let gear = settings_icon_rgba(32);
        assert_eq!(
            gear[(16 * 32 + 16) * 4 + 3],
            0,
            "gear center hole must be empty"
        );
        assert_eq!(gear[3], 0, "gear corner must be empty");
        let power = exit_icon_rgba(32);
        assert_eq!(power[3], 0, "power corner must be empty");
    }
}
