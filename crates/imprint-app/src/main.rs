mod icon;

use gpui::{
  App, AppContext as _, Bounds, QuitMode, WindowBackgroundAppearance, WindowBounds, px, size,
};
use gpui_component::{Root, TitleBar};
use imprint_ui::{ImprintApp, ImprintShell, OpenImage, OpenRaspberryPi, Quit, ToggleSettings};
use tracing_subscriber::EnvFilter;

fn main() {
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env().add_directive("imprint=info".parse().unwrap()))
    .init();

  if let Some(code) = imprint_flash::run_internal_flash() {
    std::process::exit(code);
  }

  gpui_platform::application()
    .with_quit_mode(QuitMode::LastWindowClosed)
    .with_assets(gpui_component_assets::Assets)
    .run(|cx: &mut App| {
      imprint_ui::init(cx);
      cx.set_app_identity("imprint.cdxtheme.com", "Imprint");
      icon::apply_app_icon();
      cx.on_action(|_: &Quit, cx| cx.quit());
      cx.bind_keys([
        gpui::KeyBinding::new("cmd-o", OpenImage, None),
        gpui::KeyBinding::new("ctrl-o", OpenImage, None),
        gpui::KeyBinding::new("cmd-shift-r", OpenRaspberryPi, None),
        gpui::KeyBinding::new("ctrl-shift-r", OpenRaspberryPi, None),
        gpui::KeyBinding::new("cmd-,", ToggleSettings, None),
        gpui::KeyBinding::new("ctrl-,", ToggleSettings, None),
        gpui::KeyBinding::new("cmd-q", Quit, None),
        gpui::KeyBinding::new("ctrl-q", Quit, None),
      ]);
      cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
          cx.quit();
        }
      })
      .detach();

      let bounds = Bounds::centered(None, size(px(860.), px(560.)), cx);
      let mut options = TitleBar::window_options();
      options.window_bounds = Some(WindowBounds::Windowed(bounds));
      options.window_min_size = Some(size(px(720.), px(480.)));
      options.app_id = Some("imprint.cdxtheme.com".into());
      options.window_background = if cfg!(target_os = "windows") {
        WindowBackgroundAppearance::MicaBackdrop
      } else {
        WindowBackgroundAppearance::Blurred
      };
      #[cfg(any(target_os = "linux", target_os = "freebsd"))]
      {
        options.icon = icon::window_icon();
      }
      cx.open_window(options, |window, cx| {
        let app = cx.new(|cx| ImprintApp::new(window, cx));
        let shell = cx.new(|_| ImprintShell::new(app));
        cx.new(|cx| Root::new(shell, window, cx))
      })
      .unwrap();
      cx.activate(true);
    });
}
