#[derive(Default)]
struct ButtonEdges(Option<u16>);

impl ButtonEdges {
    fn press(&mut self, buttons: u16) -> u16 {
        self.0.replace(buttons).map_or(0, |last| buttons & !last)
    }
}

#[cfg(target_os = "windows")]
pub use xinput::listen;

#[cfg(target_os = "windows")]
mod xinput {
    use std::time::Duration;

    use gpui_kit::{AnyWindowHandle, App, KeyDownEvent, KeyUpEvent, Keystroke, PlatformInput};
    use windows_sys::Win32::UI::Input::XboxController::*;

    use super::ButtonEdges;

    const BUTTON_KEYS: [(u16, &str); 12] = [
        (XINPUT_GAMEPAD_B, "escape"),
        (XINPUT_GAMEPAD_A, "enter"),
        (XINPUT_GAMEPAD_X, "b"),
        (XINPUT_GAMEPAD_Y, "t"),
        (XINPUT_GAMEPAD_BACK, "c"),
        (XINPUT_GAMEPAD_START, "n"),
        (XINPUT_GAMEPAD_LEFT_SHOULDER, "pageup"),
        (XINPUT_GAMEPAD_RIGHT_SHOULDER, "pagedown"),
        (XINPUT_GAMEPAD_DPAD_UP, "up"),
        (XINPUT_GAMEPAD_DPAD_DOWN, "down"),
        (XINPUT_GAMEPAD_DPAD_LEFT, "left"),
        (XINPUT_GAMEPAD_DPAD_RIGHT, "right"),
    ];
    const CONNECTED: Duration = Duration::from_millis(30);
    const DISCONNECTED: Duration = Duration::from_secs(1);

    pub fn listen(window: AnyWindowHandle, cx: &App) {
        cx.spawn(async move |cx| {
            let executor = cx.background_executor().clone();
            let mut slot = None;
            let mut edges = ButtonEdges::default();
            loop {
                let delay = window.update(cx, |_, window, cx| {
                    if !window.is_window_active() {
                        edges = ButtonEdges::default();
                        return DISCONNECTED;
                    }
                    let state = match slot {
                        Some(index) => controller_buttons(index).map(|buttons| (index, buttons)),
                        None => (0..XUSER_MAX_COUNT).find_map(|index| {
                            controller_buttons(index).map(|buttons| (index, buttons))
                        }),
                    };
                    let Some((index, buttons)) = state else {
                        slot = None;
                        edges = ButtonEdges::default();
                        return DISCONNECTED;
                    };
                    slot = Some(index);
                    let pressed = edges.press(buttons);
                    if let Some((_, key)) = BUTTON_KEYS.iter().find(|(mask, _)| pressed & mask != 0)
                        && let Ok(keystroke) = Keystroke::parse(key)
                    {
                        window.dispatch_event(
                            PlatformInput::KeyDown(KeyDownEvent {
                                keystroke: keystroke.clone(),
                                is_held: false,
                                prefer_character_input: false,
                            }),
                            cx,
                        );
                        window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke }), cx);
                    }
                    CONNECTED
                });
                let Ok(delay) = delay else { break };
                executor.timer(delay).await;
            }
        })
        .detach();
    }

    fn controller_buttons(index: u32) -> Option<u16> {
        let mut state = XINPUT_STATE::default();
        // XInput writes the state into this initialized, correctly sized value.
        let status = unsafe { XInputGetState(index, &mut state) };
        (status == 0).then_some(state.Gamepad.wButtons)
    }
}

#[cfg(test)]
mod tests {
    use super::ButtonEdges;

    #[test]
    fn controller_emits_presses_once_and_ignores_buttons_held_on_connect() {
        let mut edges = ButtonEdges::default();
        assert_eq!(edges.press(1), 0);
        assert_eq!(edges.press(3), 2);
        assert_eq!(edges.press(3), 0);
        assert_eq!(edges.press(0), 0);
        assert_eq!(edges.press(1), 1);
    }
}
