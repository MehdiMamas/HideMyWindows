//! DPI-scaled placement shared by the native indicator and geometry tests.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DotRect {
    pub x: i32,
    pub y: i32,
    pub size: i32,
}

pub(crate) fn place(
    caption: (i32, i32, i32, i32),
    dpi: u32,
    title_width: i32,
    native_caption: bool,
) -> Option<DotRect> {
    let (left, top, right, bottom) = caption;
    let scale = |n: i32| (n * dpi.max(96) as i32 + 48) / 96;
    let size = scale(8);
    let gap = scale(8);
    let title_start = left + gap + if native_caption { scale(24) } else { 0 };
    // Leave the caption controls unobstructed, including modern wide buttons.
    let limit = right - scale(if native_caption { 138 } else { 32 }) - gap - size;
    if bottom - top < size || limit < title_start {
        return None;
    }
    Some(DotRect {
        x: (title_start + title_width.max(0) + gap).min(limit),
        y: top + (bottom - top - size) / 2,
        size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_sits_beside_the_caption_text() {
        assert_eq!(
            place((100, 50, 900, 74), 96, 200, true),
            Some(DotRect {
                x: 340,
                y: 58,
                size: 8
            })
        );
        assert_eq!(
            place((100, 50, 900, 74), 96, 200, false),
            Some(DotRect {
                x: 316,
                y: 58,
                size: 8
            })
        );
    }

    #[test]
    fn long_titles_do_not_cover_caption_buttons() {
        let dot = place((0, 0, 800, 24), 96, 2000, true).unwrap();
        assert!(dot.x + dot.size <= 800 - 138 - 8);
    }

    #[test]
    fn tiny_windows_without_room_have_no_dot() {
        assert!(place((0, 0, 100, 24), 96, 200, true).is_none());
        assert!(place((0, 0, 800, 4), 96, 200, false).is_none());
    }

    #[test]
    fn placement_scales_with_dpi_and_handles_negative_monitor_coordinates() {
        let dot = place((-1600, -100, 0, -52), 192, 400, true).unwrap();
        assert_eq!(
            dot,
            DotRect {
                x: -1120,
                y: -84,
                size: 16
            }
        );
    }
}
