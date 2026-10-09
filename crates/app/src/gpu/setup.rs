use super::choice::{GpuChoice, pick_adapter};
use std::sync::Arc;

/// eframe options that start wgpu with `choice`'s backends and adapter rule.
pub fn native_options(choice: GpuChoice) -> eframe::NativeOptions {
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = choice.backends();
    let software = choice == GpuChoice::Software;
    setup.native_adapter_selector = Some(Arc::new(
        move |adapters: &[wgpu::Adapter], _surface: Option<&wgpu::Surface<'_>>| {
            let types: Vec<_> = adapters.iter().map(|a| a.get_info().device_type).collect();
            pick_adapter(&types, software)
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
        .with_inner_size([1200.0, 800.0])
        .with_min_inner_size([640.0, 480.0]);
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
