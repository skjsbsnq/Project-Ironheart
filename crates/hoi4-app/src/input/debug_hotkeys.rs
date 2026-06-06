use winit::event::{KeyEvent, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

pub(crate) fn is_global_debug_key(event: &WindowEvent) -> bool {
    matches!(
        event,
        WindowEvent::KeyboardInput {
            event: KeyEvent {
                physical_key: PhysicalKey::Code(
                    KeyCode::F1
                        | KeyCode::F2
                        | KeyCode::F3
                        | KeyCode::F4
                        | KeyCode::F5
                        | KeyCode::F6
                        | KeyCode::F7
                        | KeyCode::F8
                        | KeyCode::F9
                        | KeyCode::F10
                        | KeyCode::F11
                        | KeyCode::F12
                        | KeyCode::KeyR,
                ),
                ..
            },
            ..
        }
    )
}
