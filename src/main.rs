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
    let mut handheld = false;
    let mut files = Vec::new();
    let mut options = true;
    for arg in std::env::args_os().skip(1) {
        if options && arg == "--" {
            options = false;
        } else if options && arg == "--handheld" {
            handheld = true;
        } else if !options || !arg.to_string_lossy().starts_with("-psn_") {
            files.push(PathBuf::from(arg));
        }
    }
    application().with_assets(assets::AllAssets).run(move |cx| {
        #[cfg(target_os = "linux")]
        load_bundled_fonts(cx);
        init(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new(
            if cfg!(target_os = "macos") {
                "cmd-q"
            } else {
                "ctrl-q"
            },
            Quit,
            None,
        )]);
        cx.set_menus(vec![Menu {
            name: "墨页".into(),
            disabled: false,
            items: vec![MenuItem::action("退出墨页", Quit)],
        }]);
        let (preferred, minimum) = if handheld {
            (size(px(1100.), px(760.)), size(px(640.), px(400.)))
        } else {
            (size(px(1280.), px(825.)), size(px(930.), px(595.)))
        };
        let bounds = Bounds::centered(None, preferred, cx);
        let window = WindowOptions {
            window_bounds: Some(if handheld && cfg!(target_os = "linux") {
                WindowBounds::Fullscreen(bounds)
            } else {
                WindowBounds::Windowed(bounds)
            }),
            window_min_size: Some(minimum),
            titlebar: Some(TitlebarOptions {
                title: Some("墨页".into()),
                appears_transparent: cfg!(target_os = "macos"),
                traffic_light_position: Some(point(px(17.), px(21.))),
            }),
            app_id: Some("app.moye.reader".to_owned()),
            ..Default::default()
        };
        open_window(window, cx, |window, cx| {
            cx.new(|cx| reader::Reader::new(files, handheld, window, cx))
        })
        .expect("无法打开墨页窗口");
        cx.activate(true);
    });
}

#[cfg(target_os = "linux")]
fn load_bundled_fonts(cx: &mut App) {
    let Ok(executable) = std::env::current_exe() else {
        return;
    };
    let folder = executable.with_file_name("fonts");
    let fonts = ["NotoSansCJKsc-Regular.otf", "NotoSerifCJKsc-Regular.otf"]
        .into_iter()
        .filter_map(|name| std::fs::read(folder.join(name)).ok())
        .map(std::borrow::Cow::Owned)
        .collect::<Vec<_>>();
    if !fonts.is_empty()
        && let Err(error) = cx.text_system().add_fonts(fonts)
    {
        eprintln!("无法载入随附字体：{error}");
    }
}
