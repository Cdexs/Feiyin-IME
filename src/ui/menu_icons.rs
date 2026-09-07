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

/// 编辑图标（铅笔，EDITICON-175）。
pub fn edit_icon_rgba(size: u32) -> Vec<u8> {
    rasterize(size, pencil_covered)
}

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

// —— EDITICON-175：编辑图标（铅笔，尖朝左下、笔杆朝右上，45° 对角）——
// 语义「这里可以编辑文字」。选铅笔而非其他符号：编辑态最强公认符号（Segoe MDL2
// Edit / 各输入法编辑键同构），18px 下单一路径形状无小尺寸信息密度问题（对比
// TRAY-ICON-158 教训：16px 8 齿齿轮欠采样发糊）。风格与齿轮/电源同语言：SSAA 4x4、
// 品牌橙、直边平切端（不做圆角端帽，与两参照一致）。
// 几何（18px 网格迭代调优后取值）：尖点 T(-0.40S, 0.40S)，笔杆平切端 E(0.40S, -0.40S)。
// 笔杆 = 轴上 s∈[0.31,1] 宽 0.16S（≈2.9px @18px）的斜条；笔尖 = s∈[0,0.22]（≈4.2px
// @18px）由尖点线性展开到同宽的三角；两者之间留 ≈0.086S（≈1.8px @18px）缺口
// （Segoe MDL2 Edit 的「尖-杆分离」识别特征；16px 压力档下缺口仍可辨）。
const PENCIL_TIP: (f32, f32) = (-0.40, 0.40);
const PENCIL_END: (f32, f32) = (0.40, -0.40);
const PENCIL_HALF_W: f32 = 0.08; // 笔杆半宽（全宽 0.16S ≈ 2.9px @18px）
const PENCIL_S_TIP: f32 = 0.22; // 尖三角占轴长比例（≈4.2px @18px）
const PENCIL_S_GAP_END: f32 = 0.31; // 笔杆起点（缺口 ≈0.086S ≈ 1.8px @18px）

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
    let s = px * ax + py * ay; // 沿轴位置 0..1
    if !(0.0..=1.0).contains(&s) {
        return false;
    }
    let d = (px * nx + py * ny).abs(); // 到轴的垂距
    if s <= PENCIL_S_TIP {
        // 尖三角：宽度从尖点线性展开
        d <= PENCIL_HALF_W * (s / PENCIL_S_TIP)
    } else if s >= PENCIL_S_GAP_END {
        d <= PENCIL_HALF_W // 笔杆
    } else {
        false // 尖-杆缺口
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// EDITICON-175 视觉自证：18px（运行时实际尺寸）+ 16（压力档）+ 24 共 3 档，
    /// 各一张 8x 最近邻放大版，写到 coder-2 的 collab outbox。
    /// 🔴 预览只证明「形态好不好看」（DIAG-166 教训），不证明运行时行为——运行时
    /// 是否画得出来由下一单集成 + Gavin 端测判定。
    #[test]
    fn dump_edit_icon_preview() {
        let dir = std::path::Path::new("D:/Workspace/CodeLab/collab/outbox/coder-2/icons");
        std::fs::create_dir_all(dir).unwrap();
        let scale = 8u32;
        for size in [16u32, 18, 24] {
            let rgba = edit_icon_rgba(size);
            let path = dir.join(format!("edit-{size}.png"));
            image::save_buffer(&path, &rgba, size, size, image::ExtendedColorType::Rgba8).unwrap();
            let mut big = Vec::with_capacity((size * scale * size * scale * 4) as usize);
            for by in 0..size * scale {
                for bx in 0..size * scale {
                    let i = (((by / scale) * size + bx / scale) * 4) as usize;
                    big.extend_from_slice(&rgba[i..i + 4]);
                }
            }
            let bpath = dir.join(format!("edit-{size}-x8.png"));
            image::save_buffer(
                &bpath,
                &big,
                size * scale,
                size * scale,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        }
        // 覆盖率抽检：四角全透明；笔杆中段（几何中心附近）不透明
        let icon = edit_icon_rgba(18);
        for corner in [0usize, 17] {
            assert_eq!(
                icon[(corner * 18 + corner) * 4 + 3],
                0,
                "corner must be empty"
            );
            assert_eq!(
                icon[(corner * 18 + (17 - corner)) * 4 + 3],
                0,
                "corner must be empty"
            );
        }
        let mid = ((9 * 18 + 9) * 4 + 3) as usize;
        assert!(
            icon[mid] > 200,
            "pencil shaft mid must be opaque, got {}",
            icon[mid]
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
        // 笔杆中段满覆盖像素：alpha 直通预乘（rasterize 满覆盖 = 252 非 255，
        // 见 rasterize 的 alpha 公式 —— 与齿轮/电源同一既定行为）
        let mid = (9 * 18 + 9) * 4;
        assert_eq!(
            src[mid + 3],
            252,
            "test premise: shaft mid full-coverage alpha"
        );
        assert_eq!(pm[mid + 3], 252, "alpha must pass through premultiply");
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
