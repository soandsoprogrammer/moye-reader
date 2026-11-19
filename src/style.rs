use gpui_kit::{component::Icon, *};

use crate::{
    book::NoteKind,
    library::{Paper, Tone},
};

#[derive(Clone, Copy)]
pub struct Palette {
    pub paper: u32,
    pub rail: u32,
    pub panel: u32,
    pub surface: u32,
    pub hover: u32,
    pub line: u32,
    pub ink: u32,
    pub text: u32,
    pub soft: u32,
    pub dim: u32,
    pub green: u32,
    pub green_bg: u32,
    pub button: u32,
    pub white: u32,
    pub selection: u32,
    pub danger: u32,
}

impl Palette {
    pub fn for_library() -> Self {
        Self {
            paper: 0x151e1e,
            rail: 0x182221,
            panel: 0x111919,
            surface: 0x1c2828,
            hover: 0x273635,
            line: 0x334343,
            ink: 0xf0f3eb,
            text: 0xd1dcd1,
            soft: 0xbac9c3,
            dim: 0x9bafaa,
            green: 0xb6d8bd,
            green_bg: 0x294038,
            button: 0xc0dabc,
            white: 0x182a24,
            selection: 0x425c49,
            danger: 0xf09d91,
        }
    }

    pub fn for_paper(paper: Paper) -> Self {
        match paper {
            Paper::Warm => Self {
                paper: 0xfbf9f3,
                rail: 0xf3f3eb,
                panel: 0xf6f6ef,
                surface: 0xfdfcf7,
                hover: 0xe6e9df,
                line: 0xe1e3d8,
                ink: 0x242d25,
                text: 0x343b31,
                soft: 0x575f53,
                dim: 0x697165,
                green: 0x3b624d,
                green_bg: 0xe0e9dc,
                button: 0x355641,
                white: 0xfafcf6,
                selection: 0xd8e7d6,
                danger: 0xa1433c,
            },
            Paper::White => Self {
                paper: 0xffffff,
                rail: 0xf3f5f3,
                panel: 0xf9fafa,
                surface: 0xffffff,
                hover: 0xe8ece8,
                line: 0xe0e5e0,
                ink: 0x202b25,
                text: 0x303b33,
                soft: 0x535f55,
                dim: 0x667269,
                green: 0x37634a,
                green_bg: 0xe3eee5,
                button: 0x345944,
                white: 0xffffff,
                selection: 0xd4e8d8,
                danger: 0xa13a34,
            },
            Paper::Night => Self {
                paper: 0x202621,
                rail: 0x191f1b,
                panel: 0x1b221d,
                surface: 0x252d27,
                hover: 0x2b382f,
                line: 0x35423a,
                ink: 0xe7eee5,
                text: 0xd3ddd2,
                soft: 0xb4c2b5,
                dim: 0x9aab9d,
                green: 0xa5c9a7,
                green_bg: 0x34493b,
                button: 0xa6c9a8,
                white: 0x19271e,
                selection: 0x425c49,
                danger: 0xf09d91,
            },
        }
    }

    pub fn note_color(self, kind: NoteKind) -> u32 {
        match (self.paper == 0x202621, kind) {
            (_, NoteKind::Footnote) => self.danger,
            (false, NoteKind::Endnote) => 0x8b552e,
            (true, NoteKind::Endnote) => 0xe6ba80,
            (false, NoteKind::Gloss) => 0x376b71,
            (true, NoteKind::Gloss) => 0x8fc4c3,
            (false, NoteKind::Commentary) => 0x904438,
            (true, NoteKind::Commentary) => 0xde9c90,
            (false, NoteKind::Variant) => 0x77622f,
            (true, NoteKind::Variant) => 0xd6c18c,
        }
    }

    pub fn epub_ink(self, color: u32) -> Hsla {
        let mut color: Hsla = rgb(color).into();
        if color.s < 0.13 {
            return rgb(self.text).into();
        }
        color.l = if self.paper == 0x202621 {
            color.l.clamp(0.68, 0.86)
        } else {
            color.l.clamp(0.19, 0.42)
        };
        color
    }

    pub fn tone(self, tone: Tone) -> (u32, u32) {
        match (self.paper == 0x202621, tone) {
            (false, Tone::Amber) => (0xc89c43, 0xf5ebce),
            (false, Tone::Sage) => (0x769b77, 0xe4ede0),
            (false, Tone::Rose) => (0xbf7f7c, 0xf4e4df),
            (true, Tone::Amber) => (0xd8b664, 0x51482d),
            (true, Tone::Sage) => (0x98bc92, 0x394b39),
            (true, Tone::Rose) => (0xd49a99, 0x4f3a3b),
        }
    }
}

pub const SERIF: &str = if cfg!(target_os = "macos") {
    "Songti SC"
} else {
    "Noto Serif CJK SC"
};

pub fn icon(path: &'static str, size: f32, color: u32) -> Icon {
    Icon::default()
        .path(path)
        .size(px(size))
        .text_color(rgb(color))
}

pub const BOOKMARK: &str = "icons/bookmark.svg";
pub const BOOKMARKED: &str = "icons/bookmark-check.svg";
pub const BOOK: &str = "icons/book-open.svg";
pub const LEFT: &str = "icons/chevron-left.svg";
pub const RIGHT: &str = "icons/chevron-right.svg";
pub const PEN: &str = "icons/pencil.svg";
pub const CLOSE: &str = "icons/close.svg";
pub const NOTE: &str = "icons/notebook-pen.svg";
pub const LIST: &str = "icons/list.svg";
pub const LIBRARY: &str = "icons/library.svg";
pub const PANEL_RIGHT: &str = "icons/panel-right.svg";
pub const PANEL_LEFT: &str = "icons/panel-left.svg";
pub const ADD: &str = "icons/plus.svg";
pub const TYPE: &str = "icons/a-large-small.svg";
pub const COPY: &str = "icons/copy.svg";
pub const DELETE: &str = "icons/trash.svg";
pub const UNDERLINE: &str = "icons/underline.svg";
pub const LINK: &str = "icons/arrow-up-right.svg";
pub const FILE: &str = "icons/file-text.svg";

pub fn label(text: impl Into<SharedString>, size: f32, color: u32) -> Div {
    div()
        .text_size(px(size))
        .text_color(rgb(color))
        .child(text.into())
}

pub fn line(color: u32) -> Div {
    div().w_full().h(px(1.)).flex_shrink_0().bg(rgb(color))
}

pub fn badge(text: impl Into<SharedString>, p: Palette) -> Div {
    label(text, 10., p.green)
        .font_weight(FontWeight::SEMIBOLD)
        .px(px(6.))
        .py(px(2.))
        .rounded(px(4.))
        .bg(rgb(p.green_bg))
}

pub fn surface(p: Palette) -> Div {
    div()
        .border_1()
        .border_color(rgb(p.line))
        .bg(rgb(p.surface))
        .rounded(px(11.))
}
