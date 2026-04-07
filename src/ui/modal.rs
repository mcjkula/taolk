use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

pub fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}

pub fn vertical_pad(content_height: usize, area_height: u16) -> usize {
    usize::from(area_height).saturating_sub(content_height) / 2
}

pub fn horizontal_pad(text_width: usize, area_width: u16) -> String {
    " ".repeat(usize::from(area_width).saturating_sub(text_width) / 2)
}

pub fn centered_line(text: &str, area_width: u16, style: Style) -> Line<'static> {
    let pad = horizontal_pad(text.chars().count(), area_width);
    Line::styled(format!("{pad}{text}"), style)
}

pub fn centered_spans(spans: Vec<Span<'static>>, area_width: u16) -> Line<'static> {
    let width: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let mut out: Vec<Span<'static>> = Vec::with_capacity(spans.len() + 1);
    out.push(Span::raw(horizontal_pad(width, area_width)));
    out.extend(spans);
    Line::from(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    #[test]
    fn vertical_pad_centers() {
        assert_eq!(vertical_pad(10, 20), 5);
        assert_eq!(vertical_pad(0, 10), 5);
    }

    #[test]
    fn vertical_pad_clamps_when_overflow() {
        assert_eq!(vertical_pad(30, 10), 0);
    }

    #[test]
    fn horizontal_pad_centers() {
        assert_eq!(horizontal_pad(4, 10), "   ");
        assert_eq!(horizontal_pad(0, 8), "    ");
    }

    #[test]
    fn horizontal_pad_clamps_when_overflow() {
        assert_eq!(horizontal_pad(20, 10), "");
    }

    #[test]
    fn centered_line_pads_around_text() {
        let line = centered_line("hi", 10, Style::default().fg(Color::White));
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(rendered, "    hi");
    }

    #[test]
    fn centered_line_handles_unicode_width() {
        let line = centered_line("τalk", 10, Style::default());
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(rendered, "   τalk");
    }
}
