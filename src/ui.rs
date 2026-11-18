use gpui_kit::{
    component::{input::Textarea, tooltip::Tooltip},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    book::{Link, PointInText, Target},
    library::{Annotation, Bookmark, Paper, ReadingMode, Tone},
    reader::{Reader, ReadingPanel, Screen, Tab},
    style::{self, *},
};

type Control = Stateful<Div>;
const TOP_HEIGHT: Pixels = px(57.);
const FOOTER_HEIGHT: Pixels = px(56.);

fn control(id: impl Into<ElementId>, p: Palette) -> Control {
    button_style(id, p.soft, p.hover)
}

pub(crate) fn button_style(id: impl Into<ElementId>, color: u32, hover: u32) -> Control {
    div()
        .id(id)
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .px(px(10.))
        .h(px(32.))
        .rounded(px(7.))
        .text_color(rgb(color))
        .text_size(px(12.))
        .cursor_pointer()
        .focusable()
        .tab_stop(true)
        .hover(move |this| this.bg(rgb(hover)))
        .focus_visible(move |this| this.bg(rgb(hover)))
}

fn icon_control(
    id: impl Into<ElementId>,
    glyph: &'static str,
    title: &'static str,
    active: bool,
    p: Palette,
) -> Control {
    control(id, p)
        .w(px(32.))
        .px_0()
        .aria_label(title)
        .child(icon(glyph, 16., if active { p.green } else { p.soft }))
        .when(active, |this| this.bg(rgb(p.green_bg)))
        .tooltip(move |window, cx| Tooltip::new(title).build(window, cx))
}

fn primary(id: &'static str, text: &str, p: Palette) -> Control {
    button_style(id, p.white, p.green)
        .bg(rgb(p.button))
        .font_weight(FontWeight::MEDIUM)
        .px(px(13.))
        .child(text.to_owned())
}

fn segment(id: &'static str, text: impl Into<SharedString>, selected: bool, p: Palette) -> Control {
    div()
        .id(id)
        .flex_1()
        .min_w_0()
        .h(px(29.))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .rounded(px(6.))
        .cursor_pointer()
        .focusable()
        .tab_stop(true)
        .text_size(px(12.))
        .text_color(rgb(if selected { p.green } else { p.soft }))
        .when(selected, |el| {
            el.bg(rgb(p.surface))
                .font_weight(FontWeight::SEMIBOLD)
                .border_1()
                .border_color(rgb(p.line))
        })
        .when(!selected, |el| el.hover(|el| el.bg(rgb(p.hover))))
        .child(text.into())
}

fn nav_item(
    id: &'static str,
    title: &'static str,
    glyph: &'static str,
    selected: bool,
    p: Palette,
) -> Control {
    control(id, p)
        .px(px(12.))
        .h(px(32.))
        .when(selected, |el| {
            el.bg(rgb(p.green_bg)).font_weight(FontWeight::SEMIBOLD)
        })
        .child(icon(glyph, 15., if selected { p.green } else { p.dim }))
        .child(label(title, 13., if selected { p.green } else { p.soft }))
}

fn segmented(p: Palette) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(3.))
        .p(px(3.))
        .rounded(px(9.))
        .bg(rgb(p.rail))
}

impl Render for Reader {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let reading = self.screen == Screen::Reading;
        let p = if reading {
            Palette::for_paper(self.library.settings.paper)
        } else {
            Palette::for_library()
        };
        let viewport = window.viewport_size();
        let slim = viewport.width < px(1110.);
        let left = if slim { px(172.) } else { px(202.) };
        let right = if slim { px(241.) } else { px(285.) };
        let main = if reading {
            let mut width = viewport.width;
            if !self.handheld && self.panel_open(ReadingPanel::Contents) {
                width -= left;
            }
            if !self.handheld && self.panel_open(ReadingPanel::Notes) {
                width -= right;
            }
            self.sync_pagination(
                size(width, viewport.height - TOP_HEIGHT - FOOTER_HEIGHT),
                p,
                window,
                cx,
            );
            self.reading_layout(left, right, p, window, cx)
                .into_any_element()
        } else {
            self.shelf_page(window, p, cx).into_any_element()
        };

        div()
            .id("moye")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(rgb(p.paper))
            .text_color(rgb(p.ink))
            .text_size(px(13.))
            .when(cfg!(target_os = "linux"), |root| {
                root.font_family("Noto Sans CJK SC")
            })
            .flex()
            .flex_col()
            .on_key_down(cx.listener(Self::key_down))
            .capture_any_mouse_up(cx.listener(|reader, event: &MouseUpEvent, window, cx| {
                if event.button == MouseButton::Left {
                    reader.end_drag(event, window, cx);
                }
            }))
            .on_drop(cx.listener(|reader, paths: &ExternalPaths, window, cx| {
                reader.import_paths(paths.0.to_vec(), window, cx);
            }))
            .child(self.top_bar(p, cx))
            .child(main)
            .when_some(self.handheld_panel.filter(|_| reading), |root, panel| {
                root.child(self.handheld_drawer(panel, p, cx))
            })
            .when(reading && self.source_popup.is_some(), |root| {
                root.child(self.source_popup_view(window, p, cx))
            })
            .when(reading && self.settings_open, |root| {
                root.child(self.reading_settings(p, cx))
            })
            .when(
                reading
                    && self.anchor.is_some()
                    && self.selection.is_some()
                    && self.editor.is_none(),
                |root| root.child(self.selection_menu(window, p, cx)),
            )
            .when_some(self.notice.as_ref(), |root, (message, error)| {
                root.child(self.notification(message, *error, p, cx))
            })
            .when(self.editor.is_some(), |root| {
                root.child(self.note_editor(p, cx))
            })
    }
}

impl Reader {
    fn reading_layout(
        &self,
        left: Pixels,
        right: Pixels,
        p: Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .when(
                !self.handheld && self.panel_open(ReadingPanel::Contents),
                |row| row.child(self.contents_panel(left, p, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(self.page(p, window, cx))
                    .child(self.footer(p, cx)),
            )
            .when(
                !self.handheld && self.panel_open(ReadingPanel::Notes),
                |row| row.child(self.notes_panel(right, p, cx)),
            )
    }

    fn handheld_drawer(&self, panel: ReadingPanel, p: Palette, cx: &mut Context<Self>) -> Div {
        let width = px(288.);
        let drawer = div()
            .absolute()
            .top_0()
            .bottom_0()
            .w(width)
            .shadow_lg()
            .occlude();
        let drawer = match panel {
            ReadingPanel::Contents => drawer.left_0().child(self.contents_panel(width, p, cx)),
            ReadingPanel::Notes => drawer.right_0().child(self.notes_panel(width, p, cx)),
        };
        div()
            .absolute()
            .top(TOP_HEIGHT)
            .bottom(FOOTER_HEIGHT)
            .w_full()
            .child(
                div()
                    .id("dismiss-handheld-drawer")
                    .absolute()
                    .inset_0()
                    .bg(rgba(0x121b1554))
                    .occlude()
                    .on_click(
                        cx.listener(move |reader, _, _, cx| reader.set_panel(panel, false, cx)),
                    ),
            )
            .child(drawer)
    }

    fn top_bar(&self, p: Palette, cx: &Context<Self>) -> Div {
        let reading = self.screen == Screen::Reading;
        let title = div()
            .h_full()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(8.))
            .window_control_area(WindowControlArea::Drag)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(27.))
                    .rounded(px(8.))
                    .bg(rgb(p.button))
                    .child(icon(BOOK, 15., p.white)),
            )
            .child(label("墨页", 15., p.ink).font_weight(FontWeight::BOLD));
        let import = button_style("import", p.white, p.green)
            .bg(rgb(p.button))
            .px(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .when(self.handheld, |el| el.h(px(44.)))
            .child(icon(ADD, 15., p.white))
            .child(if self.busy {
                "正在载入…"
            } else {
                "导入书籍"
            })
            .on_click(cx.listener(|reader, _, window, cx| reader.choose_files(window, cx)));

        let middle = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .window_control_area(WindowControlArea::Drag)
            .when(reading, |el| {
                el.child(
                    label(self.book.info.title.clone(), 13., p.ink)
                        .min_w_0()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .font_weight(FontWeight::SEMIBOLD),
                )
                .child(badge(self.book.info.format.label(), p).flex_shrink_0())
            });

        div()
            .flex()
            .items_center()
            .h(TOP_HEIGHT)
            .flex_shrink_0()
            .pl(if cfg!(target_os = "macos") {
                px(96.)
            } else {
                px(18.)
            })
            .pr(px(19.))
            .gap(px(14.))
            .bg(rgb(p.panel))
            .border_b_1()
            .border_color(rgb(p.line))
            .child(title)
            .child(div().h(px(18.)).w(px(1.)).bg(rgb(p.line)).flex_shrink_0())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(
                        nav_item("open-library", "书库", LIBRARY, !reading, p)
                            .on_click(
                                cx.listener(|reader, _, window, cx| {
                                    reader.open_library(window, cx)
                                }),
                            )
                            .when(self.handheld, |el| el.h(px(44.))),
                    )
                    .child(
                        nav_item("open-reader", "阅读", BOOK, reading, p)
                            .on_click(
                                cx.listener(|reader, _, window, cx| reader.open_reader(window, cx)),
                            )
                            .when(self.handheld, |el| el.h(px(44.))),
                    ),
            )
            .child(middle)
            .when(!reading, |bar| bar.child(import))
            .when(reading, |bar| bar.child(self.reading_actions(p, cx)))
    }

    fn reading_actions(&self, p: Palette, cx: &Context<Self>) -> Div {
        let settings = control("reading-settings", p)
            .aria_label("阅读设置")
            .when(self.handheld, |el| el.h(px(44.)))
            .when(self.settings_open, |el| el.bg(rgb(p.green_bg)))
            .child(icon(TYPE, 17., p.green))
            .child("阅读设置")
            .on_click(cx.listener(|reader, _, _, cx| {
                reader.anchor = None;
                reader.source_popup = None;
                reader.handheld_panel = None;
                reader.settings_open = !reader.settings_open;
                cx.notify();
            }));
        let marked = self.bookmarks().contains(&self.bookmark_position());
        let bookmark = icon_control(
            "bookmark-current",
            if marked { BOOKMARKED } else { BOOKMARK },
            if marked {
                "移除此处书签（⌘D）"
            } else {
                "添加书签（⌘D）"
            },
            marked,
            p,
        )
        .when(self.handheld, |el| el.size(px(44.)))
        .on_click(cx.listener(|reader, _, window, cx| reader.toggle_bookmark(window, cx)));
        div()
            .flex()
            .items_center()
            .gap(px(5.))
            .flex_shrink_0()
            .when(self.can_go_back(), |bar| {
                bar.child(
                    icon_control(
                        "jump-back",
                        "icons/arrow-left.svg",
                        "返回跳转前的位置",
                        false,
                        p,
                    )
                    .on_click(cx.listener(|reader, _, _, cx| reader.go_back(cx))),
                )
            })
            .child(
                icon_control(
                    "toggle-sidebar",
                    PANEL_LEFT,
                    "显示或隐藏目录",
                    self.panel_open(ReadingPanel::Contents),
                    p,
                )
                .when(self.handheld, |el| el.size(px(44.)))
                .on_click(cx.listener(|reader, _, _, cx| {
                    let panel = ReadingPanel::Contents;
                    reader.set_panel(panel, !reader.panel_open(panel), cx);
                })),
            )
            .child(settings)
            .child(bookmark)
            .child(
                icon_control(
                    "toggle-notes",
                    PANEL_RIGHT,
                    "显示或隐藏批注",
                    self.panel_open(ReadingPanel::Notes),
                    p,
                )
                .when(self.handheld, |el| el.size(px(44.)))
                .on_click(cx.listener(|reader, _, _, cx| {
                    let panel = ReadingPanel::Notes;
                    reader.set_panel(panel, !reader.panel_open(panel), cx);
                })),
            )
    }

    fn contents_panel(&self, width: Pixels, p: Palette, cx: &Context<Self>) -> Div {
        let visible_item = self.visible_item().saturating_sub(1);
        let active = self.contents_entry(self.section, visible_item);
        let view = cx.weak_entity();
        let book = self.book.clone();
        let rows = uniform_list(
            "contents-scroll",
            book.contents.len(),
            move |range, _, _| {
                range
                    .map(|index| {
                        let entry = &book.contents[index];
                        let selected = active == Some(index);
                        let section = entry.section;
                        let block = entry.block;
                        let view = view.clone();
                        div().w_full().h(px(37.)).pb(px(3.)).child(
                            div()
                                .id(("chapter", index))
                                .h_full()
                                .flex()
                                .items_center()
                                .pr(px(8.))
                                .pl(px(7. + entry.depth.min(4) as f32 * 9.))
                                .rounded(px(6.))
                                .gap(px(7.))
                                .cursor_pointer()
                                .focusable()
                                .tab_stop(true)
                                .when(selected, |el| el.bg(rgb(p.green_bg)))
                                .when(!selected, |el| el.hover(|s| s.bg(rgb(p.hover))))
                                .child(
                                    div()
                                        .w(px(2.))
                                        .h(px(13.))
                                        .flex_shrink_0()
                                        .rounded_full()
                                        .when(selected, |el| el.bg(rgb(p.green))),
                                )
                                .child(
                                    label(
                                        entry.title.clone(),
                                        13.,
                                        if selected { p.green } else { p.soft },
                                    )
                                    .min_w_0()
                                    .whitespace_nowrap()
                                    .text_ellipsis()
                                    .when(selected, |el| el.font_weight(FontWeight::SEMIBOLD)),
                                )
                                .on_click(move |_, _, app| {
                                    let _ = view.update(app, |reader, cx| {
                                        reader.navigate(section, block, cx)
                                    });
                                }),
                        )
                    })
                    .collect::<Vec<_>>()
            },
        )
        .track_scroll(&self.contents_scroll)
        .flex_1()
        .min_h_0()
        .w_full();

        div()
            .w(width)
            .h_full()
            .flex_shrink_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(p.rail))
            .border_r_1()
            .border_color(rgb(p.line))
            .px(px(13.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(56.))
                    .px(px(6.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(7.))
                            .child(icon(LIST, 15., p.green))
                            .child(label("目录", 14., p.ink).font_weight(FontWeight::SEMIBOLD)),
                    )
                    .child(label(self.book.contents.len().to_string(), 11., p.dim)),
            )
            .child(rows)
    }

    fn footer(&self, p: Palette, cx: &mut Context<Self>) -> Div {
        let current = self.progress();
        let percent = format!("{:.0}%", current * 100.);
        let before = self.can_turn(false);
        let after = self.can_turn(true);
        let paged = self.is_paged();
        let (previous_title, next_title) = if paged {
            ("上一页", "下一页")
        } else {
            ("上一章", "下一章")
        };
        let previous = control("previous", p)
            .px(px(8.))
            .when(self.handheld, |el| el.h(px(44.)).px(px(12.)))
            .aria_label(previous_title)
            .child(icon(LEFT, 15., p.soft))
            .child(previous_title)
            .when(!before, |b| b.opacity(0.4).cursor_default().tab_stop(false))
            .on_click(cx.listener(|reader, _, _, cx| reader.turn(false, cx)));
        let next = control("next", p)
            .px(px(8.))
            .when(self.handheld, |el| el.h(px(44.)).px(px(12.)))
            .aria_label(next_title)
            .child(next_title)
            .child(icon(RIGHT, 15., p.soft))
            .when(!after, |b| b.opacity(0.4).cursor_default().tab_stop(false))
            .on_click(cx.listener(|reader, _, _, cx| reader.turn(true, cx)));
        div()
            .h(FOOTER_HEIGHT)
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(14.))
            .px(px(17.))
            .border_t_1()
            .border_color(rgb(p.line))
            .bg(rgb(p.panel))
            .child(previous)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        label(
                            if paged {
                                let pages = self.visible_pages();
                                let start = pages.start + 1;
                                let current = if start < pages.end {
                                    format!("{start}–{}", pages.end)
                                } else {
                                    start.to_string()
                                };
                                format!("本章 {current} / {}", self.page_count())
                            } else {
                                "阅读进度".to_owned()
                            },
                            11.,
                            p.dim,
                        )
                        .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .flex_1()
                            .h(px(4.))
                            .rounded_full()
                            .overflow_hidden()
                            .bg(rgb(p.line))
                            .child(
                                div()
                                    .w(relative(if current > 0. { current.max(0.015) } else { 0. }))
                                    .h_full()
                                    .rounded_full()
                                    .bg(rgb(p.green)),
                            ),
                    )
                    .child(
                        label(percent, 11., p.green)
                            .w(px(34.))
                            .text_right()
                            .font_weight(FontWeight::SEMIBOLD),
                    ),
            )
            .child(next)
    }

    fn notes_panel(&self, width: Pixels, p: Palette, cx: &mut Context<Self>) -> Div {
        let sources = &self.data().notes;
        let personal = self.annotations();
        let bookmarks = self.bookmarks();
        let header = div()
            .flex()
            .items_center()
            .gap(px(9.))
            .mb(px(15.))
            .child(
                div()
                    .size(px(31.))
                    .rounded(px(8.))
                    .bg(rgb(p.green_bg))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon(NOTE, 16., p.green)),
            )
            .child(
                label("阅读记录", 16., p.ink)
                    .font_weight(FontWeight::BOLD)
                    .flex_1(),
            )
            .child(
                icon_control("close-notes", CLOSE, "关闭批注栏", false, p).on_click(cx.listener(
                    |reader, _, _, cx| {
                        reader.set_panel(ReadingPanel::Notes, false, cx);
                    },
                )),
            );
        let tabs = segmented(p)
            .child(
                segment("source-notes-tab", "原书注释", self.tab == Tab::Source, p).on_click(
                    cx.listener(|reader, _, _, cx| {
                        reader.tab = Tab::Source;
                        cx.notify();
                    }),
                ),
            )
            .child(
                segment(
                    "personal-notes-tab",
                    "我的批注",
                    self.tab == Tab::Personal,
                    p,
                )
                .on_click(cx.listener(|reader, _, _, cx| {
                    reader.tab = Tab::Personal;
                    cx.notify();
                })),
            )
            .child(
                segment("bookmarks-tab", "书签", self.tab == Tab::Bookmarks, p).on_click(
                    cx.listener(|reader, _, _, cx| {
                        reader.tab = Tab::Bookmarks;
                        cx.notify();
                    }),
                ),
            );

        let mut cards = div()
            .id("notes-scroll")
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(10.));
        if self.tab == Tab::Bookmarks {
            if bookmarks.is_empty() {
                cards = cards.child(self.empty_notes(
                    BOOKMARK,
                    "还没有书签",
                    "点击工具栏的书签图标，记下这里。",
                    p,
                ));
            } else {
                cards = cards.children(
                    bookmarks
                        .iter()
                        .rev()
                        .enumerate()
                        .map(|(index, bookmark)| self.bookmark_card(index, *bookmark, p, cx)),
                );
            }
        } else if self.tab == Tab::Source {
            if sources.is_empty() {
                cards = cards.child(self.empty_notes(
                    NOTE,
                    "本章没有原书注释",
                    "脚注、尾注和词条会显示在这里。",
                    p,
                ));
            } else {
                let view = cx.entity();
                let list = list(self.source_list.clone(), move |index, _, app| {
                    view.read(app).source_card(index, p, &view)
                })
                .w_full()
                .flex_1()
                .min_h_0();
                cards = cards.child(list);
            }
        } else if personal.is_empty() {
            cards = cards.child(self.empty_notes(
                UNDERLINE,
                "还没有批注",
                "拖选正文，就能划线或添加备注。",
                p,
            ));
        } else {
            cards = cards.children(
                personal
                    .iter()
                    .rev()
                    .map(|note| self.personal_card(note, p, cx)),
            );
        }

        div()
            .w(width)
            .h_full()
            .flex_shrink_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(p.panel))
            .border_l_1()
            .border_color(rgb(p.line))
            .pt(px(19.))
            .px(px(16.))
            .pb(px(15.))
            .child(header)
            .child(tabs)
            .child(
                div().px(px(3.)).mt(px(21.)).mb(px(9.)).child(
                    label(
                        match self.tab {
                            Tab::Source => format!("本章注释 · {}", sources.len()),
                            Tab::Personal => format!("本书划线与备注 · {}", personal.len()),
                            Tab::Bookmarks => format!("本书书签 · {}", bookmarks.len()),
                        },
                        11.,
                        p.dim,
                    )
                    .font_weight(FontWeight::SEMIBOLD),
                ),
            )
            .child(cards)
            .child(line(p.line))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .px(px(3.))
                    .pt(px(12.))
                    .child(icon("icons/hard-drive.svg", 13., p.dim))
                    .child(label("书签与批注保存在本机", 11., p.dim)),
            )
    }

    fn source_card(&self, index: usize, p: Palette, view: &Entity<Self>) -> AnyElement {
        let Some(target) = self.data().notes.get(index) else {
            return div().h_0().into_any_element();
        };
        let Some(note) = self.book.notes.get(target) else {
            return div().h_0().into_any_element();
        };
        let active = self.source.as_ref() == Some(target);
        let target = target.clone();
        let jump = target.clone();
        let has_reference = self.data().note_locations.contains_key(&jump);
        let body = if active {
            note.text.clone()
        } else {
            shortened(note.text.as_ref(), 95)
        };
        let accent = p.note_color(note.kind);
        let toggle = view.downgrade();
        let navigator = view.downgrade();

        div()
            .pb(px(10.))
            .child(
                style::surface(p)
                    .p(px(12.))
                    .when(active, |card| card.border_color(rgb(accent)))
                    .child(
                        div()
                            .id(("note-source", index))
                            .w_full()
                            .flex()
                            .items_center()
                            .gap(px(7.))
                            .cursor_pointer()
                            .focusable()
                            .tab_stop(true)
                            .child(icon(NOTE, 14., accent))
                            .child(
                                label(note.title.clone(), 12., accent)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .flex_1(),
                            )
                            .child(icon(
                                if active {
                                    "icons/chevron-up.svg"
                                } else {
                                    "icons/chevron-down.svg"
                                },
                                13.,
                                p.dim,
                            ))
                            .on_click(move |_, _, app| {
                                let _ = toggle.update(app, |reader, cx| {
                                    let old = reader.source.take();
                                    if old.as_ref() != Some(&target) {
                                        reader.source = Some(target.clone());
                                    }
                                    if let Some(old_index) = old.as_ref().and_then(|old| {
                                        reader.data().notes.iter().position(|item| item == old)
                                    }) {
                                        reader
                                            .source_list
                                            .remeasure_items(old_index..old_index + 1);
                                    }
                                    reader.source_list.remeasure_items(index..index + 1);
                                    cx.notify();
                                });
                            }),
                    )
                    .child(label(body, 13., p.text).mt(px(8.)).line_height(px(22.)))
                    .when(has_reference, |card| {
                        card.child(
                            control(("source-jump", index), p)
                                .mt(px(7.))
                                .h(px(25.))
                                .px_0()
                                .justify_start()
                                .child(icon(LINK, 13., accent))
                                .child("定位引用")
                                .on_click(move |_, _, app| {
                                    let _ = navigator
                                        .update(app, |reader, cx| reader.jump_to_source(&jump, cx));
                                }),
                        )
                    }),
            )
            .into_any_element()
    }

    fn bookmark_card(
        &self,
        index: usize,
        bookmark: Bookmark,
        p: Palette,
        cx: &Context<Self>,
    ) -> Div {
        let section = self.book.sections.get(bookmark.section);
        let title = section
            .map(|section| section.title.clone())
            .unwrap_or_else(|| "原章节已不可用".into());
        let excerpt = section
            .and_then(|section| {
                let first = if bookmark.item == 0 {
                    usize::from(section.header == Some(0))
                } else {
                    bookmark.item - 1
                };
                section
                    .blocks
                    .iter()
                    .enumerate()
                    .skip(first)
                    .find_map(|(index, block)| {
                        let text = if index == first {
                            block.text.get(bookmark.byte..).unwrap_or(&block.text)
                        } else {
                            &block.text
                        }
                        .trim();
                        (!text.is_empty()).then_some(text)
                    })
            })
            .map(|text| shortened(text, 70));

        style::surface(p)
            .p(px(9.))
            .flex_shrink_0()
            .flex()
            .items_start()
            .gap(px(4.))
            .child(
                div()
                    .id(("jump-bookmark", index))
                    .cursor_pointer()
                    .focusable()
                    .tab_stop(true)
                    .flex_1()
                    .min_w_0()
                    .rounded(px(5.))
                    .px(px(5.))
                    .py(px(4.))
                    .hover(|el| el.bg(rgb(p.hover)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(icon(BOOKMARK, 13., p.green))
                            .child(
                                label(title, 13., p.ink)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .min_w_0()
                                    .whitespace_nowrap()
                                    .text_ellipsis(),
                            ),
                    )
                    .when_some(excerpt, |card, excerpt| {
                        card.child(label(excerpt, 12., p.soft).line_height(px(20.)).mt(px(6.)))
                    })
                    .on_click(
                        cx.listener(move |reader, _, _, cx| reader.jump_to_bookmark(bookmark, cx)),
                    ),
            )
            .child(
                icon_control(
                    ("delete-bookmark", index),
                    "icons/bookmark-minus.svg",
                    "移除书签",
                    false,
                    p,
                )
                .size(px(25.))
                .on_click(
                    cx.listener(move |reader, _, _, cx| reader.delete_bookmark(bookmark, cx)),
                ),
            )
    }

    fn source_popup_view(&self, window: &Window, p: Palette, cx: &Context<Self>) -> Div {
        let Some((target, anchor)) = &self.source_popup else {
            return div();
        };
        let Some(note) = self.book.notes.get(target) else {
            return div();
        };
        let accent = p.note_color(note.kind);
        let size = window.viewport_size();
        let width = px(382.);
        let margin = px(14.);
        let gap = px(13.);
        let below = (size.height - anchor.y - gap - px(63.)).max(px(0.));
        let above = (anchor.y - gap - px(64.)).max(px(0.));
        let open_below = below >= px(220.) || below >= above;
        let height = (if open_below { below } else { above }).clamp(px(100.), px(300.));
        let left = (anchor.x - px(38.)).clamp(margin, (size.width - width - margin).max(margin));

        style::surface(p)
            .absolute()
            .left(left)
            .w(width)
            .max_h(height)
            .when(open_below, |card| card.top(anchor.y + gap))
            .when(!open_below, |card| {
                card.bottom(size.height - anchor.y + gap)
            })
            .shadow_lg()
            .border_color(rgb(p.line))
            .occlude()
            .flex()
            .flex_col()
            .on_mouse_down_out(cx.listener(|reader, _, _, cx| {
                reader.source_popup = None;
                cx.notify();
            }))
            .child(
                div()
                    .px(px(14.))
                    .pt(px(9.))
                    .pb(px(4.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .child(icon(NOTE, 15., accent))
                    .child(
                        label(note.title.clone(), 12., accent)
                            .font_weight(FontWeight::SEMIBOLD)
                            .flex_1(),
                    )
                    .child(
                        icon_control("close-source-popup", CLOSE, "关闭原书注释", false, p)
                            .size(px(25.))
                            .on_click(cx.listener(|reader, _, _, cx| {
                                reader.source_popup = None;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("source-popup-scroll")
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(15.))
                    .pb(px(14.))
                    .child(
                        label(note.text.clone(), 16., p.text)
                            .font_family(SERIF)
                            .line_height(px(26.)),
                    ),
            )
    }

    fn jump_to_source(&mut self, target: &Target, cx: &mut Context<Self>) {
        if let Some(&block) = self.data().note_locations.get(target) {
            let byte = self.data().blocks[block]
                .spans
                .iter()
                .find(|span| matches!(&span.style.link, Some(Link::Local(link)) if link == target))
                .map_or(0, |span| span.range.start);
            self.navigate_to_text(self.section, PointInText { block, byte }, cx);
            self.source = Some(target.clone());
            cx.notify();
        }
    }

    fn empty_notes(
        &self,
        glyph: &'static str,
        title: &'static str,
        hint: &'static str,
        p: Palette,
    ) -> Div {
        style::surface(p)
            .flex_shrink_0()
            .p(px(15.))
            .flex()
            .flex_col()
            .gap(px(7.))
            .child(icon(glyph, 19., p.green))
            .child(label(title, 13., p.ink).font_weight(FontWeight::SEMIBOLD))
            .child(label(hint, 12., p.dim).line_height(px(20.)))
    }

    fn personal_card(&self, note: &Annotation, p: Palette, cx: &Context<Self>) -> Div {
        let id = note.id;
        let (under, shade) = p.tone(note.tone);
        let section = self
            .book
            .sections
            .get(note.passage.section)
            .map(|data| data.title.clone())
            .unwrap_or_default();
        style::surface(p)
            .p(px(11.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .when(self.revealed == Some(id), |card| {
                card.border_color(rgb(under))
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .size(px(7.))
                            .rounded_full()
                            .bg(rgb(under))
                            .flex_shrink_0(),
                    )
                    .child(
                        label(section, 11., p.dim)
                            .flex_1()
                            .min_w_0()
                            .whitespace_nowrap()
                            .text_ellipsis(),
                    )
                    .child(
                        icon_control(("edit-note", id as usize), PEN, "编辑备注", false, p)
                            .size(px(24.))
                            .on_click(cx.listener(move |reader, _, window, cx| {
                                reader.edit_annotation(Some(id), window, cx)
                            })),
                    )
                    .child(
                        icon_control(
                            ("delete-note", id as usize),
                            DELETE,
                            "删除划线和备注",
                            false,
                            p,
                        )
                        .size(px(24.))
                        .on_click(
                            cx.listener(move |reader, _, _, cx| reader.delete_annotation(id, cx)),
                        ),
                    ),
            )
            .child(
                div()
                    .id(("reveal-note", id as usize))
                    .mt(px(6.))
                    .p(px(8.))
                    .rounded(px(5.))
                    .bg(rgb(shade))
                    .cursor_pointer()
                    .focusable()
                    .tab_stop(true)
                    .child(
                        label(shortened(&note.quote, 110), 13., p.text)
                            .font_family(SERIF)
                            .line_height(px(21.)),
                    )
                    .on_click(cx.listener(move |reader, _, window, cx| {
                        reader.reveal_annotation(id, window, cx)
                    })),
            )
            .when(!note.body.is_empty(), |card| {
                card.child(
                    label(note.body.clone(), 13., p.ink)
                        .line_height(px(21.))
                        .mt(px(8.)),
                )
            })
    }

    fn reading_settings(&self, p: Palette, cx: &Context<Self>) -> Div {
        let heading = div()
            .flex()
            .items_center()
            .justify_between()
            .child(label("阅读设置", 14., p.ink).font_weight(FontWeight::SEMIBOLD))
            .child(
                icon_control("close-settings", CLOSE, "关闭阅读设置", false, p)
                    .size(px(26.))
                    .on_click(cx.listener(|reader, _, _, cx| {
                        reader.settings_open = false;
                        cx.notify();
                    })),
            );
        style::surface(p)
            .absolute()
            .top(px(64.))
            .right(px(45.))
            .w(px(266.))
            .p(px(14.))
            .shadow_lg()
            .occlude()
            .child(heading)
            .child(label("阅读方式", 12., p.dim).mt(px(9.)).mb(px(6.)))
            .child(
                segmented(p).children(
                    [
                        ("scroll-mode", ReadingMode::Scroll, "滚动"),
                        ("page-mode", ReadingMode::Paged, "单页"),
                        ("spread-mode", ReadingMode::Spread, "双页"),
                    ]
                    .into_iter()
                    .map(|(id, mode, title)| {
                        segment(id, title, self.reading_mode() == mode, p).on_click(cx.listener(
                            move |reader, _, window, cx| reader.set_reading_mode(mode, window, cx),
                        ))
                    }),
                ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .mt(px(11.))
                    .child(label("正文字号", 12., p.dim).flex_1())
                    .child(
                        icon_control("smaller", "icons/minus.svg", "缩小字号", false, p)
                            .on_click(cx.listener(|reader, _, _, cx| reader.size_by(-1, cx))),
                    )
                    .child(
                        label(format!("{}", self.library.settings.size), 13., p.ink)
                            .w(px(29.))
                            .text_center(),
                    )
                    .child(
                        icon_control("larger", ADD, "放大字号", false, p)
                            .on_click(cx.listener(|reader, _, _, cx| reader.size_by(1, cx))),
                    ),
            )
            .child(div().my(px(10.)).child(line(p.line)))
            .child(
                segmented(p).children(
                    [
                        ("warm", Paper::Warm, "纸色"),
                        ("white", Paper::White, "明亮"),
                        ("night", Paper::Night, "夜间"),
                    ]
                    .into_iter()
                    .map(|(id, paper, title)| {
                        segment(id, title, self.library.settings.paper == paper, p).on_click(
                            cx.listener(move |reader, _, _, cx| reader.set_paper(paper, cx)),
                        )
                    }),
                ),
            )
    }

    fn swatches(&self, current: Tone, editor: bool, p: Palette, cx: &Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(3.))
            .children(Tone::ALL.into_iter().enumerate().map(|(index, tone)| {
                let (stroke, shade) = p.tone(tone);
                div()
                    .id((if editor { "editor-tone" } else { "quick-tone" }, index))
                    .size(px(24.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_1()
                    .border_color(if current == tone {
                        rgb(stroke)
                    } else {
                        rgba(0x00000000)
                    })
                    .cursor_pointer()
                    .focusable()
                    .tab_stop(true)
                    .aria_label(tone.title())
                    .tooltip(move |window, cx| Tooltip::new(tone.title()).build(window, cx))
                    .child(
                        div()
                            .size(px(14.))
                            .rounded_full()
                            .border_1()
                            .border_color(rgb(stroke))
                            .bg(rgb(shade)),
                    )
                    .on_click(cx.listener(move |reader, _, _, cx| {
                        if editor {
                            if let Some(editing) = reader.editor.as_mut() {
                                editing.tone = tone;
                            }
                        } else {
                            reader.chosen_tone = tone;
                        }
                        cx.notify();
                    }))
            }))
    }

    fn selection_menu(&self, window: &Window, p: Palette, cx: &Context<Self>) -> Div {
        let at = self.anchor.unwrap_or_default();
        let width = px(318.);
        let position = window.viewport_size();
        style::surface(p)
            .w(width)
            .absolute()
            .left((at.x - px(144.)).clamp(px(8.), (position.width - width - px(8.)).max(px(8.))))
            .top((at.y - px(48.)).clamp(px(60.), (position.height - px(108.)).max(px(60.))))
            .p(px(4.))
            .flex()
            .items_center()
            .justify_between()
            .shadow_lg()
            .occlude()
            .child(self.swatches(self.chosen_tone, false, p, cx).px(px(4.)))
            .child(div().w(px(1.)).h(px(18.)).bg(rgb(p.line)))
            .child(
                control("quick-underline", p)
                    .child(icon(UNDERLINE, 14., p.green))
                    .child("划线")
                    .on_click(
                        cx.listener(|reader, _, window, cx| reader.add_underline(window, cx)),
                    ),
            )
            .child(
                control("quick-note", p)
                    .child(icon(PEN, 14., p.green))
                    .child("备注")
                    .on_click(cx.listener(|reader, _, window, cx| {
                        reader.edit_annotation(None, window, cx)
                    })),
            )
            .child(
                control("quick-copy", p)
                    .child(icon(COPY, 14., p.green))
                    .child("复制")
                    .on_click(
                        cx.listener(|reader, _, window, cx| reader.copy_selection(window, cx)),
                    ),
            )
    }

    fn note_editor(&self, p: Palette, cx: &Context<Self>) -> Div {
        let editor = self.editor.as_ref().expect("note editor is open");
        div()
            .absolute()
            .inset_0()
            .size_full()
            .bg(rgba(0x18221866))
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .child(
                style::surface(p)
                    .w(px(450.))
                    .p(px(20.))
                    .shadow_lg()
                    .occlude()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .mb(px(13.))
                            .child(
                                label(
                                    if editor.id.is_some() {
                                        "编辑备注"
                                    } else {
                                        "添加备注"
                                    },
                                    17.,
                                    p.ink,
                                )
                                .font_weight(FontWeight::BOLD),
                            )
                            .child(
                                icon_control("close-editor", CLOSE, "取消备注", false, p).on_click(
                                    cx.listener(|reader, _, window, cx| {
                                        reader.close_editor(window, cx)
                                    }),
                                ),
                            ),
                    )
                    .child(
                        label(shortened(&editor.quote, 150), 14., p.text)
                            .font_family(SERIF)
                            .line_height(px(22.))
                            .p(px(9.))
                            .mb(px(12.))
                            .rounded(px(6.))
                            .bg(rgb(p.tone(editor.tone).1)),
                    )
                    .child(Textarea::new(&self.input))
                    .child(
                        div()
                            .mt(px(14.))
                            .flex()
                            .items_center()
                            .gap(px(7.))
                            .child(self.swatches(editor.tone, true, p, cx))
                            .child(div().flex_1())
                            .child(control("cancel-editor", p).child("取消").on_click(
                                cx.listener(|reader, _, window, cx| {
                                    reader.close_editor(window, cx)
                                }),
                            ))
                            .child(primary("save-editor", "保存备注", p).on_click(cx.listener(
                                |reader, _, window, cx| reader.commit_editor(window, cx),
                            ))),
                    ),
            )
    }

    fn notification(&self, message: &str, error: bool, p: Palette, cx: &Context<Self>) -> Div {
        style::surface(p)
            .absolute()
            .bottom(px(69.))
            .right(px(17.))
            .max_w(px(440.))
            .max_h(px(180.))
            .flex()
            .items_start()
            .gap(px(8.))
            .p(px(9.))
            .shadow_md()
            .occlude()
            .child(icon(
                if error {
                    "icons/triangle-alert.svg"
                } else {
                    "icons/check.svg"
                },
                16.,
                if error { p.danger } else { p.green },
            ))
            .child(
                label(shortened(message, 193), 12., p.ink)
                    .line_height(px(20.))
                    .min_w_0(),
            )
            .child(
                icon_control("dismiss-notice", CLOSE, "关闭提示", false, p)
                    .size(px(22.))
                    .on_click(cx.listener(|reader, _, _, cx| {
                        reader.notice = None;
                        cx.notify();
                    })),
            )
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        if self.editor.is_some() {
            match key {
                "escape" => {
                    self.close_editor(window, cx);
                    cx.stop_propagation();
                }
                "enter" if modifiers.secondary(cx) => {
                    self.commit_editor(window, cx);
                    cx.stop_propagation();
                }
                _ => {}
            }
            return;
        }
        if key == "o" && modifiers.secondary(cx) {
            self.choose_files(window, cx);
            cx.stop_propagation();
            return;
        }
        if key == "l" && modifiers.secondary(cx) {
            if self.screen == Screen::Reading {
                self.open_library(window, cx);
            } else {
                self.open_reader(window, cx);
            }
            cx.stop_propagation();
            return;
        }
        if self.handheld && !modifiers.modified() && self.handheld_key(key, window, cx) {
            cx.stop_propagation();
            return;
        }
        if self.screen != Screen::Reading {
            return;
        }
        match key {
            "c" if modifiers.secondary(cx) => self.copy_selection(window, cx),
            "d" if modifiers.secondary(cx) => self.toggle_bookmark(window, cx),
            "=" | "+" if modifiers.secondary(cx) => self.size_by(1, cx),
            "-" if modifiers.secondary(cx) => self.size_by(-1, cx),
            "escape" => {
                self.settings_open = false;
                self.selection = None;
                self.anchor = None;
                self.source = None;
                self.source_popup = None;
                cx.notify();
            }
            "left" if !modifiers.modified() => self.turn(false, cx),
            "right" if !modifiers.modified() => self.turn(true, cx),
            "pageup" => self.page_scroll(false, window, cx),
            "pagedown" | "space" => self.page_scroll(!modifiers.shift, window, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn handheld_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if key == "f11" {
            window.toggle_fullscreen();
            return true;
        }
        if self.screen == Screen::Library {
            match key {
                "left" | "up" => window.focus_prev(cx),
                "right" | "down" => window.focus_next(cx),
                "enter" if self.focus.is_focused(window) && !self.busy => {
                    self.open_reader(window, cx);
                }
                _ => return false,
            }
            return true;
        }
        match key {
            "escape" if self.source_popup.take().is_some() => cx.notify(),
            "escape" if std::mem::take(&mut self.settings_open) => cx.notify(),
            "escape" if self.handheld_panel.take().is_some() => cx.notify(),
            "escape" if self.selection.take().is_some() => {
                self.anchor = None;
                cx.notify();
            }
            "escape" => self.open_library(window, cx),
            "c" | "n" => {
                let panel = if key == "c" {
                    ReadingPanel::Contents
                } else {
                    ReadingPanel::Notes
                };
                self.set_panel(panel, !self.panel_open(panel), cx);
            }
            "b" => self.toggle_bookmark(window, cx),
            "t" => {
                self.handheld_panel = None;
                self.source_popup = None;
                self.settings_open = !self.settings_open;
                cx.notify();
            }
            "up" if self.handheld_panel.is_some() => window.focus_prev(cx),
            "down" if self.handheld_panel.is_some() => window.focus_next(cx),
            _ => return false,
        }
        true
    }
}

fn shortened(text: &str, limit: usize) -> SharedString {
    match text.char_indices().nth(limit) {
        Some((byte, _)) => format!("{}…", text[..byte].trim_end()).into(),
        None => text.to_owned().into(),
    }
}
