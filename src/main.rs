use std::fs::File;

use layershellev::keyboard::{KeyCode, PhysicalKey};
use layershellev::reexport::*;
use layershellev::*;

use state::WgpuState;

mod state;

fn main() {
    let ev: WindowState<()> = WindowState::new("Interactive Wallpaper")
        .with_allscreens()
        .with_layer(Layer::Background)
        .with_anchor(Anchor::Top | Anchor::Left | Anchor::Right | Anchor::Bottom)
        .with_exclusive_zone(-1)
        .with_keyboard_interacivity(KeyboardInteractivity::None)
        .with_use_display_handle(true)
        .build()
        .unwrap();

    let mut mouse_pos = (0.0, 0.0);
    let mut state: WgpuState = ev
        .get_unit_iter()
        .next()
        .map(|unit| pollster::block_on(WgpuState::new(unit)))
        .expect("wgpu init failed")
        .expect("no window state unit");

    ev.running(move |event, ev, index| match event {
        LayerShellEvent::InitRequest => ReturnData::RequestBind,
        LayerShellEvent::BindProvide(_, _) => ReturnData::RequestCompositor,
        LayerShellEvent::CompositorProvide(_, _) => ReturnData::None,

        LayerShellEvent::NormalDispatch => {
            ReturnData::None
        }

        LayerShellEvent::RequestMessages(&DispatchMessage::RequestRefresh {
            width,
            height,
            ..
        }) => {
            state.resize(width, height);
            state.render();

            ReturnData::None
        }

        LayerShellEvent::RequestMessages(DispatchMessage::MouseButton { .. }) => ReturnData::None,
        LayerShellEvent::RequestMessages(DispatchMessage::MouseEnter { pointer, .. }) => {
            ReturnData::RequestSetCursorShape(("crosshair".to_owned(), pointer.clone()))
        }

        LayerShellEvent::RequestMessages(&DispatchMessage::MouseMotion {
            time,
            surface_x,
            surface_y,
        }) => {
            println!("{time}, {surface_x}, {surface_y}");
            mouse_pos = (surface_x, surface_y);
            ev.request_refresh_all(RefreshRequest::NextFrame);
            ReturnData::None
        }

        LayerShellEvent::RequestMessages(DispatchMessage::KeyboardInput { event, .. }) => {
            if let PhysicalKey::Code(KeyCode::Escape) = event.physical_key {
                ReturnData::RequestExit
            } else {
                ReturnData::None
            }
        }

        _ => ReturnData::None,
    })
    .unwrap();
}
