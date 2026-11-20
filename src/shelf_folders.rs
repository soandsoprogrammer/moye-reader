use gpui_kit::{
    component::input::{Input, InputEvent, InputState},
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    reader::Reader,
    style::{self, CLOSE, Palette, icon, label},
    ui::button_style,
};

pub(crate) const FOLDER: &str = "icons/folder.svg";
pub(crate) const FOLDER_PLUS: &str = "icons/folder-plus.svg";

pub(crate) struct ShelfDialog {
    focus: FocusHandle,
    kind: ShelfDialogKind,
}

enum ShelfDialogKind {
    Name {
        folder: Option<u64>,
        input: Entity<InputState>,
        error: Option<String>,
        _submit: Subscription,
    },
    Move,
    Remove {
        folder: u64,
        name: String,
    },
}

impl Reader {
    pub(crate) fn show_shelf_folder(
        &mut self,
        folder: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clear_shelf_selection();
        self.shelf_folder = folder.filter(|id| self.library.folder(*id).is_some());
        self.focus.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn name_shelf_folder(
        &mut self,
        folder: Option<u64>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current = match folder {
            Some(id) => {
                let Some(folder) = self.library.folder(id) else {
                    return;
                };
                folder.name.clone()
            }
            None => String::new(),
        };
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("文件夹名称")
                .default_value(current)
        });
        let submit = cx.subscribe_in(
            &input,
            window,
            |reader, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    reader.save_folder_name(window, cx);
                }
            },
        );
        let focus = cx.focus_handle();
        self.shelf_dialog = Some(ShelfDialog {
            focus,
            kind: ShelfDialogKind::Name {
                folder,
                input: input.clone(),
                error: None,
                _submit: submit,
            },
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    fn save_folder_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ShelfDialog {
            kind:
                ShelfDialogKind::Name {
                    folder,
                    input,
                    error,
                    ..
                },
            ..
        }) = self.shelf_dialog.as_mut()
        else {
            return;
        };
        let move_books = folder.is_none() && !self.shelf_selected.is_empty();
        match self
            .library
            .name_folder(*folder, input.read(cx).value().as_str())
        {
            Ok(id) => {
                if move_books {
                    match self.library.move_books(&self.shelf_selected, Some(id)) {
                        Ok(()) => self.clear_shelf_selection(),
                        Err(error) => self.tell(error.to_string(), true, window, cx),
                    }
                }
                self.close_shelf_dialog(window, cx);
                self.save_now(cx);
            }
            Err(message) => {
                *error = Some(message.to_string());
                cx.notify();
            }
        }
    }

    pub(crate) fn move_selected_books(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.shelf_selected.is_empty() {
            return;
        }
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        self.shelf_dialog = Some(ShelfDialog {
            focus,
            kind: ShelfDialogKind::Move,
        });
        cx.notify();
    }

    pub(crate) fn remove_shelf_folder(
        &mut self,
        folder: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(info) = self.library.folder(folder) else {
            return;
        };
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        self.shelf_dialog = Some(ShelfDialog {
            focus,
            kind: ShelfDialogKind::Remove {
                folder,
                name: info.name.clone(),
            },
        });
        cx.notify();
    }

    fn close_shelf_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.shelf_dialog = None;
        self.focus.focus(window, cx);
        cx.notify();
    }

    pub(crate) fn shelf_dialog_key(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if key == "escape" {
            self.close_shelf_dialog(window, cx);
            return true;
        }
        let Some(dialog) = self.shelf_dialog.as_ref() else {
            return false;
        };
        if !self.handheld || matches!(dialog.kind, ShelfDialogKind::Name { .. }) {
            return false;
        }
        let previous = window.focused(cx);
        match key {
            "left" | "up" => window.focus_prev(cx),
            "right" | "down" => window.focus_next(cx),
            _ => return false,
        }
        if !dialog.focus.contains_focused(window, cx)
            && let Some(previous) = previous
        {
            previous.focus(window, cx);
        }
        true
    }

    pub(crate) fn shelf_dialog_view(
        &self,
        dialog: &ShelfDialog,
        window: &Window,
        p: Palette,
        cx: &Context<Self>,
    ) -> Div {
        let viewport = window.viewport_size();
        let (title, content) = match &dialog.kind {
            ShelfDialogKind::Name {
                folder,
                input,
                error,
                ..
            } => {
                let creating = folder.is_none();
                (
                    if creating {
                        "新建文件夹"
                    } else {
                        "重命名文件夹"
                    },
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .child(Input::new(input))
                        .when_some(error.as_ref(), |form, error| {
                            form.child(label(error.clone(), 12., p.danger))
                        })
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap(px(8.))
                                .child(dialog_cancel(p, cx))
                                .child(
                                    button_style("folder-save", p.white, p.green)
                                        .bg(rgb(p.button))
                                        .child(if creating { "新建" } else { "保存" })
                                        .on_click(cx.listener(|reader, _, window, cx| {
                                            reader.save_folder_name(window, cx)
                                        })),
                                ),
                        ),
                )
            }
            ShelfDialogKind::Move => {
                let current = self.shelf_folder;
                (
                    "移至文件夹",
                    div()
                        .flex()
                        .flex_col()
                        .min_h_0()
                        .gap(px(8.))
                        .child(label(
                            format!("已选 {} 本", self.shelf_selected.len()),
                            13.,
                            p.soft,
                        ))
                        .child(
                            div()
                                .id("move-folder-list")
                                .min_h_0()
                                .overflow_y_scroll()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .max_h((viewport.height - px(180.)).min(px(315.)))
                                .children(
                                    std::iter::once((None, "书库首页"))
                                        .chain(
                                            self.library.folders.iter().map(|folder| {
                                                (Some(folder.id), folder.name.as_str())
                                            }),
                                        )
                                        .map(|(id, name)| {
                                            button_style(
                                                ("move-shelf-folder", id.unwrap_or_default()),
                                                p.ink,
                                                p.hover,
                                            )
                                            .h(px(39.))
                                            .w_full()
                                            .justify_start()
                                            .child(icon(FOLDER, 16., p.green))
                                            .child(
                                                label(name.to_owned(), 13., p.ink)
                                                    .flex_1()
                                                    .min_w_0()
                                                    .whitespace_nowrap()
                                                    .text_ellipsis(),
                                            )
                                            .when(current == id, |row| {
                                                row.child(icon("icons/check.svg", 15., p.green))
                                            })
                                            .on_click(
                                                cx.listener(move |reader, _, window, cx| {
                                                    match reader
                                                        .library
                                                        .move_books(&reader.shelf_selected, id)
                                                    {
                                                        Ok(()) => {
                                                            reader.clear_shelf_selection();
                                                            reader.close_shelf_dialog(window, cx);
                                                            reader.save_now(cx);
                                                        }
                                                        Err(error) => reader.tell(
                                                            error.to_string(),
                                                            true,
                                                            window,
                                                            cx,
                                                        ),
                                                    }
                                                }),
                                            )
                                        }),
                                ),
                        )
                        .child(
                            button_style("move-new-folder", p.green, p.hover)
                                .justify_start()
                                .child(icon(FOLDER_PLUS, 16., p.green))
                                .child("新建文件夹并移入")
                                .on_click(cx.listener(|reader, _, window, cx| {
                                    reader.name_shelf_folder(None, window, cx)
                                })),
                        ),
                )
            }
            ShelfDialogKind::Remove { folder, name } => {
                let folder = *folder;
                (
                    "移除文件夹",
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(14.))
                        .child(label(
                            format!("「{name}」中的书籍将移回书库首页，阅读记录会保留。"),
                            13.,
                            p.text,
                        ))
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap(px(8.))
                                .child(dialog_cancel(p, cx))
                                .child(
                                    button_style("folder-remove-confirm", p.danger, p.hover)
                                        .bg(rgb(p.rail))
                                        .child("移除文件夹")
                                        .on_click(cx.listener(move |reader, _, window, cx| {
                                            reader.library.remove_folder(folder);
                                            reader.show_shelf_folder(None, window, cx);
                                            reader.close_shelf_dialog(window, cx);
                                            reader.save_now(cx);
                                        })),
                                ),
                        ),
                )
            }
        };
        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x121b1577))
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .child(
                style::surface(p)
                    .w((viewport.width - px(32.)).min(px(440.)))
                    .p(px(16.))
                    .shadow_lg()
                    .occlude()
                    .tab_group()
                    .track_focus(&dialog.focus)
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(label(title, 16., p.ink).font_weight(FontWeight::SEMIBOLD))
                            .child(
                                button_style("folder-dialog-close", p.soft, p.hover)
                                    .aria_label("关闭")
                                    .px(px(7.))
                                    .child(icon(CLOSE, 15., p.soft))
                                    .on_click(cx.listener(|reader, _, window, cx| {
                                        reader.close_shelf_dialog(window, cx)
                                    })),
                            ),
                    )
                    .child(content),
            )
    }
}

fn dialog_cancel(p: Palette, cx: &Context<Reader>) -> Stateful<Div> {
    button_style("folder-cancel", p.soft, p.hover)
        .child("取消")
        .on_click(cx.listener(|reader, _, window, cx| reader.close_shelf_dialog(window, cx)))
}
