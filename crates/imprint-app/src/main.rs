use gpui::{
  App, Bounds, Menu, MenuItem, QuitMode, TitlebarOptions, WindowAppearance, WindowBounds,
  WindowOptions, point, px, size,
};
use imprint_ui::{ImprintApp, OpenImage, Quit};
use tracing_subscriber::EnvFilter;

fn main() {
  tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env().add_directive("imprint=info".parse().unwrap()))
    .init();

  gpui_platform::application()
    .with_quit_mode(QuitMode::LastWindowClosed)
    .run(|cx: &mut App| {
      cx.set_app_identity("imprint.cdxtheme.com", "Imprint");
      cx.set_window_appearance(Some(WindowAppearance::Dark));
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

      let bounds = Bounds::centered(None, size(px(1080.), px(720.)), cx);
      cx.open_window(
        WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          window_min_size: Some(size(px(860.), px(560.))),
          titlebar: Some(TitlebarOptions {
            title: Some("Imprint".into()),
            appears_transparent: false,
            traffic_light_position: Some(point(px(14.), px(18.))),
          }),
          ..Default::default()
        },
        |window, cx| cx.new(|cx| ImprintApp::new(window, cx)),
      )
      .unwrap();
      cx.activate(true);
    });
}
