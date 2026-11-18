use std::ops::Range;

use gpui_kit::{Pixels, Size, px};

pub fn page_range(page: usize, count: usize, per_turn: usize) -> Range<usize> {
    let first = page / per_turn * per_turn;
    first..(first + per_turn).min(count)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Anchor {
    pub item: usize,
    pub byte: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct LineStart {
    pub byte: usize,
    pub offset: Pixels,
}

pub struct Row {
    pub height: Pixels,
    pub lines: Vec<LineStart>,
}

impl Row {
    pub fn offset_for(&self, byte: usize) -> Pixels {
        self.lines
            .iter()
            .rev()
            .find(|line| line.byte <= byte)
            .map_or(Pixels::ZERO, |line| line.offset)
    }

    pub fn byte_for(&self, offset: Pixels) -> usize {
        self.lines
            .iter()
            .rev()
            .find(|line| line.offset <= offset)
            .map_or(0, |line| line.byte)
    }
}

#[derive(Debug)]
pub struct Fragment {
    pub start: Anchor,
    pub end_byte: Option<usize>,
    pub offset: Pixels,
    pub height: Pixels,
}

pub struct Pagination {
    pub area: Size<Pixels>,
    pub font_size: u16,
    pub rows: Vec<Row>,
    pub pages: Vec<Vec<Fragment>>,
}

impl Pagination {
    pub fn new(area: Size<Pixels>, font_size: u16, rows: Vec<Row>) -> Self {
        let mut pages = vec![Vec::new()];
        let mut available = area.height;
        for (item, row) in rows.iter().enumerate() {
            let mut start = LineStart {
                byte: 0,
                offset: Pixels::ZERO,
            };
            while start.offset < row.height {
                let limit = start.offset + available;
                let split = if row.height <= limit {
                    None
                } else {
                    let count = row.lines.partition_point(|line| line.offset <= limit);
                    match count.checked_sub(1).map(|i| row.lines[i]) {
                        Some(line) if line.offset > start.offset => Some(line),
                        _ if !pages.last().unwrap().is_empty() => {
                            pages.push(Vec::new());
                            available = area.height;
                            continue;
                        }
                        // Atomic rows are sized to fit a whole page. Keep the cursor
                        // moving if an unexpectedly short window cannot fit one.
                        _ => None,
                    }
                };
                let end = split.map_or(row.height, |line| line.offset);
                let height = end - start.offset;
                pages.last_mut().unwrap().push(Fragment {
                    start: Anchor {
                        item,
                        byte: start.byte,
                    },
                    end_byte: split.map(|line| line.byte),
                    offset: start.offset,
                    height: height.min(area.height),
                });
                if let Some(next) = split {
                    start = next;
                    pages.push(Vec::new());
                    available = area.height;
                } else {
                    available = (available - height).max(px(0.));
                    break;
                }
            }
        }
        Self {
            area,
            font_size,
            rows,
            pages,
        }
    }

    pub fn page_for(&self, anchor: Anchor) -> usize {
        self.pages
            .partition_point(|page| page.first().is_some_and(|part| part.start <= anchor))
            .saturating_sub(1)
    }
}
