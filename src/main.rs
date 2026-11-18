mod book;
mod demo;
mod epub;
mod epub_style;

mod library;
mod pictures;
mod reader;
mod shelf_view;

mod pagination;
mod reading_view;
mod style;
#[cfg(test)]
mod tests;
mod ui;

use std::path::PathBuf;

use gpui_kit::*;

actions!(moye, [Quit]);

fn main() {
    let files = std::env::args_os()
        .skip(1)
        .filter(|arg| !arg.to_string_lossy().starts_with("-psn_"))
        .map(PathBuf::from)
        .collect();
    application().with_assets(assets::AllAssets).run(move |cx| {
        init(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.set_menus(vec![Menu {
            name: "墨页".into(),
            disabled: false,
            items: vec![MenuItem::action("退出墨页", Quit)],
        }]);
        let bounds = Bounds::centered(None, size(px(1280.), px(825.)), cx);
        let window = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(930.), px(595.))),
            titlebar: Some(TitlebarOptions {
                title: Some("墨页".into()),
                appears_transparent: cfg!(target_os = "macos"),
                traffic_light_position: Some(point(px(17.), px(21.))),
            }),
            app_id: Some("app.moye.reader".to_owned()),
            ..Default::default()
        };
        open_window(window, cx, |window, cx| {
            cx.new(|cx| reader::Reader::new(files, window, cx))
        })
        .expect("无法打开墨页窗口");
        cx.activate(true);
    });
}
