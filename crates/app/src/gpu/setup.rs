use super::choice::{GpuChoice, Os, pick_adapter};
use std::sync::Arc;

/// eframe options that start wgpu with `choice`'s backends and adapter rule.
pub fn native_options(choice: GpuChoice) -> eframe::NativeOptions {
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = choice.backends(Os::current());
    let software = choice == GpuChoice::Software;
    setup.native_adapter_selector = Some(Arc::new(
        move |adapters: &[wgpu::Adapter], _surface: Option<&wgpu::Surface<'_>>| {
            let infos: Vec<_> = adapters.iter().map(wgpu::Adapter::get_info).collect();
            let found: Vec<_> = infos.iter().map(|i| (i.device_type, i.backend)).collect();
            let listed: Vec<_> = infos
                .iter()
                .map(|i| (&i.name, i.device_type, i.backend))
                .collect();
            crate::startup_log::stage(format_args!("graphics adapters: {listed:?}"));
            let picked = pick_adapter(&found, software);
            crate::startup_log::stage(format_args!(
                "picked for {choice:?}: {:?}",
                picked.map(|i| &infos[i].name)
            ));
            picked
                .map(|i| adapters[i].clone())
                .ok_or_else(|| format!("no graphics adapter for {choice:?}"))
        },
    ));
    setup.device_descriptor = Arc::new(|adapter: &wgpu::Adapter| wgpu::DeviceDescriptor {
        label: Some("OpenDrape"),
        required_limits: wgpu::Limits::downlevel_webgl2_defaults()
            .using_resolution(adapter.limits()),
        ..Default::default()
    });

    let mut options = eframe::NativeOptions::default();
    // macOS gives every app a default menu whose Quit item (Cmd+Q) ends the process without
    // a close request, so unsaved work would be lost without a question. Without that menu,
    // Cmd+Q reaches the app as an ordinary key press, and it asks first.
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        options.event_loop_builder = Some(Box::new(|builder| {
            builder.with_default_menu(false);
        }));
    }
    options.wgpu_options.wgpu_setup = egui_wgpu::WgpuSetup::CreateNew(setup);
    options.viewport = egui::ViewportBuilder::default()
        .with_title("OpenDrape")
        .with_app_id("org.opendrape.OpenDrape")
        .with_inner_size([1000.0, 640.0])
        .with_maximized(true)
        .with_min_inner_size([640.0, 480.0])
        .with_icon(
            eframe::icon_data::from_png_bytes(include_bytes!("../../../../assets/icon@2x.png"))
                .unwrap_or_default(),
        );
    options
}

/// Tell the user the graphics could not start: stderr always, plus a dialog when `dialog`.
pub fn show_startup_error(details: &str, dialog: bool) {
    let body = crate::tr!("startup-failed", error = details.to_owned());
    eprintln!("{body}");
    if dialog {
        message_dialog(&body, true);
    }
}

/// An informational message before the window opens: stderr always, plus a dialog when `dialog`.
pub fn show_notice(text: &str, dialog: bool) {
    eprintln!("{text}");
    if dialog {
        message_dialog(text, false);
    }
}

/// Blocks until the user presses OK.
#[cfg(any(windows, target_os = "macos"))]
fn message_dialog(text: &str, error: bool) {
    let _ = rfd::MessageDialog::new()
        .set_title(crate::tr!("app-name"))
        .set_description(text)
        .set_level(if error {
            rfd::MessageLevel::Error
        } else {
            rfd::MessageLevel::Info
        })
        .show();
}

/// No dialog library on Linux: stderr only.
#[cfg(not(any(windows, target_os = "macos")))]
fn message_dialog(_text: &str, _error: bool) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_starts_maximised_so_it_fits_small_laptop_screens() {
        // 1366×768 at 125% scaling leaves ~1093×570 points: a fixed 1200×800 window would
        // hang off the bottom of the screen on the most common budget laptops.
        assert_eq!(
            native_options(GpuChoice::Auto).viewport.maximized,
            Some(true)
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_default_menu_is_off_so_cmd_q_reaches_the_unsaved_changes_question() {
        assert!(native_options(GpuChoice::Auto).event_loop_builder.is_some());
    }
}
