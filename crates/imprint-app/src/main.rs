use gpui::{
  App, AppContext as _, Bounds, Menu, MenuItem, QuitMode, WindowBackgroundAppearance, WindowBounds,
  px, size,
};
use gpui_component::{Root, TitleBar};
use imprint_ui::{ImprintApp, ImprintShell, OpenImage, Quit};
use tracing_subscriber::EnvFilter;

fn main() {
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env().add_directive("imprint=info".parse().unwrap()))
    .init();

  gpui_platform::application()
    .with_quit_mode(QuitMode::LastWindowClosed)
    .with_assets(gpui_component_assets::Assets)
    .run(|cx: &mut App| {
      imprint_ui::init(cx);
      cx.set_app_identity("imprint.cdxtheme.com", "Imprint");
      cx.on_action(|_: &Quit, cx| cx.quit());
      cx.set_menus([Menu::new("Imprint").items([
        MenuItem::action("Open Image…", OpenImage),
        MenuItem::separator(),
        MenuItem::action("Quit", Quit),
      ])]);
      cx.bind_keys([
        gpui::KeyBinding::new("cmd-o", OpenImage, None),
        gpui::KeyBinding::new("ctrl-o", OpenImage, None),
        gpui::KeyBinding::new("cmd-q", Quit, None),
        gpui::KeyBinding::new("ctrl-q", Quit, None),
      ]);
      cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
          cx.quit();
        }
      })
      .detach();

      let bounds = Bounds::centered(None, size(px(780.), px(540.)), cx);
      let mut options = TitleBar::window_options();
      options.window_bounds = Some(WindowBounds::Windowed(bounds));
      options.window_min_size = Some(size(px(640.), px(440.)));
      options.window_background = if cfg!(target_os = "windows") {
        WindowBackgroundAppearance::MicaBackdrop
      } else {
        WindowBackgroundAppearance::Blurred
      };
      cx.open_window(options, |window, cx| {
        let app = cx.new(|cx| ImprintApp::new(window, cx));
        let shell = cx.new(|_| ImprintShell::new(app));
        cx.new(|cx| Root::new(shell, window, cx))
      })
      .unwrap();
      cx.activate(true);
    });
}
