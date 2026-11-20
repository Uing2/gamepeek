use windows::Win32::UI::WindowsAndMessaging::{CreateIconFromResourceEx, HICON, LR_DEFAULTCOLOR};

const ICON_BYTES: &[u8] = include_bytes!("../assets/app.ico");

fn le_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn le_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

/// 读取 ICO 里最接近目标尺寸的图像数据。
fn pick(size: i32) -> Option<&'static [u8]> {
    if ICON_BYTES.len() < 6 || le_u16(ICON_BYTES, 2) != 1 {
        return None;
    }

    let count = le_u16(ICON_BYTES, 4) as usize;
    let mut best: Option<(i32, &'static [u8])> = None;

    for index in 0..count {
        let entry = 6 + index * 16;
        if entry + 16 > ICON_BYTES.len() {
            break;
        }

        let width = if ICON_BYTES[entry] == 0 {
            256
        } else {
            ICON_BYTES[entry] as i32
        };
        let length = le_u32(ICON_BYTES, entry + 8) as usize;
        let offset = le_u32(ICON_BYTES, entry + 12) as usize;
        if offset + length > ICON_BYTES.len() {
            continue;
        }

        let data = &ICON_BYTES[offset..offset + length];
        let score = (width - size).abs();

        // 优先选不小于目标的尺寸，其次再按差值靠近。
        let better = match best {
            None => true,
            Some((best_score, _)) => {
                let prefers_larger = width >= size && best_score > 0;
                prefers_larger || score < best_score
            }
        };

        if better {
            best = Some((score, data));
        }
    }

    best.map(|(_, data)| data)
}

pub fn load(size: i32) -> Option<HICON> {
    let data = pick(size)?;
    unsafe {
        CreateIconFromResourceEx(
            data,
            true,
            0x0003_0000,
            size,
            size,
            LR_DEFAULTCOLOR,
        )
        .ok()
    }
}