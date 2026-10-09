use super::choice::{GpuChoice, Os, pick_adapter};
use std::sync::Arc;

/// eframe options that start wgpu with `choice`'s backends and adapter rule.
pub fn native_options(choice: GpuChoice) -> eframe::NativeOptions {
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = choice.backends(Os::current());
    let software = choice == GpuChoice::Software;
    setup.native_adapter_selector = Some(Arc::new(
        move |adapters: &[wgpu::Adapter], _surface: Option<&wgpu::Surface<'_>>| {
            let found: Vec<_> = adapters
                .iter()
                .map(|a| {
                    let info = a.get_info();
                    (info.device_type, info.backend)
                })
                .collect();
            pick_adapter(&found, software)
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

/// Tell the user the graphics could not start (dialog on Windows/macOS, stderr everywhere).
pub fn show_startup_error(details: &str) {
    let body = crate::tr!("startup-failed", error = details.to_owned());
    eprintln!("{body}");
    #[cfg(any(windows, target_os = "macos"))]
    {
        let _ = rfd::MessageDialog::new()
            .set_title(crate::tr!("app-name"))
            .set_description(body)
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}

/// An informational message before the window opens (dialog on Windows/macOS, stderr everywhere).
pub fn show_notice(text: &str) {
    eprintln!("{text}");
    #[cfg(any(windows, target_os = "macos"))]
    {
        let _ = rfd::MessageDialog::new()
            .set_title(crate::tr!("app-name"))
            .set_description(text)
            .set_level(rfd::MessageLevel::Info)
            .show();
    }
}

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
}
