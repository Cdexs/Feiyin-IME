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

#[cfg(test)]
mod tests {
    use super::*;

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
