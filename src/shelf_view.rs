use gpui_kit::{prelude::FluentBuilder as _, *};

use crate::{
    book::{BookInfo, DEMO_ID},
    demo,
    reader::{ImageState, Reader},
    style::{ADD, BOOK, FILE, Palette, RIGHT, SERIF, icon, label},
    ui::button_style,
};

impl Reader {
    pub fn shelf_page(&self, window: &Window, p: Palette, cx: &Context<Self>) -> Div {
        let size = window.viewport_size();
        let gutter = if self.handheld { px(22.) } else { px(44.) };
        let width = (size.width - gutter * 2.).min(px(1120.));
        let columns = if width >= px(1000.) {
            4
        } else if width >= px(720.) || !self.handheld {
            3
        } else {
            2
        };
        let banner_height = if self.handheld && size.height < px(640.) {
            144.
        } else if size.height < px(690.) {
            198.
        } else {
            255.
        };
        let book_count = self.library.books.len() + 1;
        let show_drop_tile = self.library.books.is_empty();
        let rows = (book_count + usize::from(show_drop_tile)).div_ceil(columns);
        let featured = self
            .library
            .active
            .as_ref()
            .and_then(|id| self.library.books.iter().find(|info| &info.id == id))
            .cloned()
            .unwrap_or_else(demo::info);

        let view = cx.weak_entity();
        let shelf = uniform_list("shelf-grid", rows, move |range, window, app| {
            let Some(entity) = view.upgrade() else {
                return vec![];
            };
            let reader = entity.read(app);
            range
                .map(|row| {
                    div()
                        .w_full()
                        .h(px(127.))
                        .flex()
                        .items_stretch()
                        .gap(px(13.))
                        .pb(px(13.))
                        .children((0..columns).map(|column| {
                            let index = row * columns + column;
                            let element = if index < book_count {
                                let info = reader
                                    .library
                                    .books
                                    .iter()
                                    .rev()
                                    .nth(index)
                                    .cloned()
                                    .unwrap_or_else(demo::info);
                                reader.shelf_card(index, info, p, window, &view)
                            } else if show_drop_tile && index == book_count {
                                drop_tile(p, &view)
                            } else {
                                div().into_any_element()
                            };
                            div().flex_1().min_w_0().h_full().child(element)
                        }))
                })
                .collect::<Vec<_>>()
        })
        .w_full()
        .flex_1()
        .min_h_0();

        div()
            .flex_1()
            .min_h_0()
            .w_full()
            .flex()
            .flex_col()
            .items_center()
            .px(gutter)
            .child(
                div()
                    .w_full()
                    .max_w(px(1120.))
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .pt(px(if self.handheld { 18. } else { 27. }))
                    .child(
                        label("书库", 30., p.ink)
                            .font_weight(FontWeight::SEMIBOLD)
                            .line_height(px(38.))
                            .mb(px(16.)),
                    )
                    .child(self.featured_card(featured, banner_height, p, cx))
                    .child(
                        div()
                            .mt(px(26.))
                            .mb(px(13.))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(label("全部书籍", 16., p.ink).font_weight(FontWeight::SEMIBOLD))
                            .child(label(format!("{book_count} 本"), 12., p.dim)),
                    )
                    .child(shelf),
            )
    }

    fn featured_card(&self, info: BookInfo, height: f32, p: Palette, cx: &Context<Self>) -> Div {
        let returning = self.library.active.as_deref() == Some(info.id.as_str());
        let detail = if returning && info.id == self.book.info.id {
            self.data().title.clone()
        } else if info.id == DEMO_ID {
            "EPUB · 含脚注与尾注".into()
        } else if info.author.is_empty() {
            format!("{} 文本", info.format.label()).into()
        } else {
            info.author.clone().into()
        };
        let caption = if returning {
            "最近阅读"
        } else {
            "内置试读"
        };
        let cta = if self.busy {
            "正在载入…"
        } else if returning {
            "继续阅读"
        } else {
            "开始阅读"
        };

        div()
            .relative()
            .w_full()
            .h(px(height))
            .flex_shrink_0()
            .overflow_hidden()
            .rounded(px(16.))
            .bg(rgb(0x233b3f))
            .border_1()
            .border_color(rgba(0x6984846c))
            .when_some(self.shelf_scene.as_ref(), |card, image| {
                card.child(
                    img(image.clone())
                        .absolute()
                        .inset_0()
                        .size_full()
                        .object_fit(ObjectFit::Cover),
                )
            })
            .child(div().absolute().inset_0().size_full().bg(linear_gradient(
                90.,
                linear_color_stop(rgba(0x0c181ce8), 0.0),
                linear_color_stop(rgba(0x10202200), 0.88),
            )))
            .child(
                div()
                    .relative()
                    .h_full()
                    .p(px(32.))
                    .max_w(px(625.))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .items_start()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(7.))
                            .child(div().size(px(6.)).rounded_full().bg(rgb(0xd2af70)))
                            .child(label(caption, 12., 0xe0dcc3).font_weight(FontWeight::SEMIBOLD)),
                    )
                    .child(
                        label(info.title.clone(), 39., 0xf7f4e7)
                            .font_family(SERIF)
                            .font_weight(FontWeight::BOLD)
                            .line_height(px(53.))
                            .w_full()
                            .whitespace_nowrap()
                            .text_ellipsis(),
                    )
                    .child(
                        label(detail, 13., 0xd1ddd7)
                            .w_full()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .mb(px(15.)),
                    )
                    .child(
                        button_style("featured-read", p.white, 0xd5e7c9)
                            .bg(rgb(p.button))
                            .h(px(35.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .px(px(12.))
                            .child(cta)
                            .child(icon(RIGHT, 14., p.white))
                            .on_click(cx.listener(move |reader, _, window, cx| {
                                reader.switch_book(info.clone(), window, cx)
                            })),
                    ),
            )
    }

    fn shelf_card(
        &self,
        index: usize,
        info: BookInfo,
        p: Palette,
        window: &Window,
        view: &WeakEntity<Self>,
    ) -> AnyElement {
        let status = self.library.reading.get(&info.id).map(|book| book.position);
        let active = self.library.active.as_deref() == Some(info.id.as_str());
        let progress = status.map_or(0., |pos| pos.progress).clamp(0., 1.);
        let caption = if progress >= 0.995 {
            "已读完".to_owned()
        } else if progress > 0. {
            format!("已读 {:.0}%", (progress * 100.).max(1.))
        } else if active || status.is_some_and(|pos| pos.item != 0 || pos.offset != 0.) {
            "阅读中".to_owned()
        } else if info.id == DEMO_ID {
            "内置试读".to_owned()
        } else if !info.author.is_empty() {
            info.author.clone()
        } else {
            "未开始".to_owned()
        };
        let thumbnail = self.shelf_cover(&info, p, window, view);
        let view = view.clone();
        div()
            .id(("shelf-book", index))
            .size_full()
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(11.))
            .rounded(px(11.))
            .bg(rgb(p.surface))
            .border_1()
            .border_color(rgb(if active { 0x577564 } else { p.line }))
            .cursor_pointer()
            .focusable()
            .tab_stop(true)
            .aria_label(format!("阅读 {}", info.title))
            .hover(|this| this.bg(rgb(p.hover)).border_color(rgb(0x688878)))
            .focus_visible(|this| this.bg(rgb(p.hover)).border_color(rgb(0x688878)))
            .child(thumbnail)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(5.))
                    .pr(px(2.))
                    .child(
                        label(info.title.clone(), 14., p.ink)
                            .line_height(px(18.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .whitespace_nowrap()
                            .text_ellipsis(),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                label(info.format.label(), 10., p.green)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .flex_shrink_0(),
                            )
                            .child(
                                div()
                                    .size(px(3.))
                                    .rounded_full()
                                    .bg(rgb(p.dim))
                                    .flex_shrink_0(),
                            )
                            .child(
                                label(caption, 11., p.dim)
                                    .min_w_0()
                                    .whitespace_nowrap()
                                    .text_ellipsis(),
                            ),
                    )
                    .when(progress > 0., |card| {
                        card.child(
                            div()
                                .h(px(2.))
                                .mt(px(5.))
                                .w_full()
                                .rounded_full()
                                .overflow_hidden()
                                .bg(rgb(p.line))
                                .child(
                                    div()
                                        .h_full()
                                        .w(relative(progress.max(0.02)))
                                        .bg(rgb(p.green)),
                                ),
                        )
                    }),
            )
            .on_click(move |_, window, app| {
                let _ = view.update(app, |reader, cx| {
                    reader.switch_book(info.clone(), window, cx)
                });
            })
            .into_any_element()
    }

    fn shelf_cover(
        &self,
        info: &BookInfo,
        p: Palette,
        window: &Window,
        view: &WeakEntity<Self>,
    ) -> Div {
        let base = div()
            .relative()
            .w(px(49.))
            .h(px(72.))
            .flex_shrink_0()
            .overflow_hidden()
            .rounded(px(5.))
            .border_1()
            .border_color(rgb(p.line));
        if let Some(path) = &info.cover {
            match self.picture(info, path) {
                Some(ImageState::Ready(image)) => {
                    return base.child(
                        img(image.art.clone())
                            .size_full()
                            .object_fit(ObjectFit::Cover),
                    );
                }
                None => {
                    let info = info.clone();
                    let path = path.clone();
                    let view = view.clone();
                    window.on_next_frame(move |_, app| {
                        let _ =
                            view.update(app, |reader, cx| reader.request_picture(info, path, cx));
                    });
                }
                _ => {}
            }
        }
        if info.id == DEMO_ID {
            base.bg(rgb(0x2f4a49))
                .when_some(self.shelf_scene.as_ref(), |cover, image| {
                    cover.child(
                        img(image.clone())
                            .absolute()
                            .inset_0()
                            .size_full()
                            .object_fit(ObjectFit::Cover),
                    )
                })
                .child(div().absolute().inset_0().bg(rgba(0x15282950)))
                .child(
                    label("雾岸\n来信", 13., 0xf4edd5)
                        .line_height(px(15.))
                        .font_family(SERIF)
                        .font_weight(FontWeight::BOLD)
                        .absolute()
                        .left(px(6.))
                        .top(px(8.)),
                )
        } else {
            base.bg(rgb(p.green_bg))
                .flex()
                .items_center()
                .justify_center()
                .child(icon(
                    if info.format == crate::book::Format::Txt {
                        FILE
                    } else {
                        BOOK
                    },
                    20.,
                    p.green,
                ))
        }
    }
}

fn drop_tile(p: Palette, view: &WeakEntity<Reader>) -> AnyElement {
    let view = view.clone();
    div()
        .id("shelf-import")
        .size_full()
        .px(px(15.))
        .flex()
        .items_center()
        .gap(px(11.))
        .rounded(px(11.))
        .border_1()
        .border_color(rgb(p.line))
        .cursor_pointer()
        .focusable()
        .tab_stop(true)
        .aria_label("导入本地书籍")
        .hover(|el| el.bg(rgb(p.hover)))
        .child(
            div()
                .size(px(35.))
                .rounded(px(8.))
                .flex()
                .items_center()
                .justify_center()
                .bg(rgb(p.green_bg))
                .child(icon(ADD, 17., p.green)),
        )
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(label("导入本地书籍", 13., p.ink).font_weight(FontWeight::SEMIBOLD))
                .child(label("拖入 TXT 或 EPUB 文件", 11., p.dim)),
        )
        .on_click(move |_, window, app| {
            let _ = view.update(app, |reader, cx| reader.choose_files(window, cx));
        })
        .into_any_element()
}
