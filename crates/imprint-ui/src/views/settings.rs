use gpui::{App, Entity, InteractiveElement, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_component::{
  ActiveTheme as _, Colorize as _, IconName, Sizable as _, WindowExt as _,
  button::{Button, ButtonRounded},
  menu::{DropdownMenu as _, PopupMenuItem},
  separator::Separator,
  switch::Switch,
  tab::{Tab, TabBar},
  v_flex,
};
use imprint_core::i18n::{self, t};
use imprint_core::{Language, LocalePref, Settings};

use crate::app::ImprintApp;
use crate::theme::{Appearance, glass};
use crate::widgets::{glass_panel, glass_surface, muted, section_label};

pub(crate) fn open(view: Entity<ImprintApp>, window: &mut Window, cx: &mut App) {
  // Defer so the sheet builder is not invoked while ImprintApp is updating.
  window.defer(cx, move |window, cx| {
    window.open_sheet(cx, move |sheet, _, cx| {
      let app = view.read(cx);
      glass_panel(sheet, cx)
        .title(t("settings.title"))
        .size(px(380.))
        .child(
          v_flex()
            .gap_5()
            .py_3()
            .child(
              v_flex()
                .gap_2()
                .child(section_label(cx, t("settings.appearance")))
                .child(
                  glass_surface(v_flex().w_full().gap_3().px_4().py_4(), cx)
                    .child(
                      TabBar::new("appearance")
                        .segmented()
                        .small()
                        .w_full()
                        .selected_index(app.appearance.as_index())
                        .child(Tab::new().label(t("settings.appearance_system")))
                        .child(Tab::new().label(t("settings.appearance_light")))
                        .child(Tab::new().label(t("settings.appearance_dark")))
                        .on_click({
                          let view = view.clone();
                          move |ix, window, cx| {
                            let appearance = Appearance::from_index(*ix);
                            view.update(cx, |this, cx| this.set_appearance(appearance, window, cx));
                          }
                        }),
                    )
                    .child(muted(cx, t("settings.appearance_hint"))),
                ),
            )
            .child(
              v_flex()
                .gap_2()
                .child(section_label(cx, t("settings.language")))
                .child(
                  glass_surface(v_flex().w_full().gap_3().px_4().py_4(), cx)
                    .child(locale_dropdown(i18n::active_language(), view.clone(), cx))
                    .child(muted(cx, t("settings.language_hint"))),
                ),
            )
            .child(
              v_flex()
                .gap_2()
                .child(section_label(cx, t("settings.writing")))
                .child(
                  glass_surface(v_flex().w_full(), cx)
                    .child(setting_switch(
                      "verify",
                      t("settings.verify"),
                      t("settings.verify_hint"),
                      app.settings.verify,
                      view.clone(),
                      |s, on| s.verify = on,
                      cx,
                    ))
                    .child(Separator::horizontal())
                    .child(setting_switch(
                      "unmount",
                      t("settings.eject"),
                      t("settings.eject_hint"),
                      app.settings.unmount_on_success,
                      view.clone(),
                      |s, on| s.unmount_on_success = on,
                      cx,
                    ))
                    .child(Separator::horizontal())
                    .child(setting_switch(
                      "hide-system",
                      t("settings.hide_system"),
                      t("settings.hide_system_hint"),
                      app.settings.hide_system_drives,
                      view.clone(),
                      |s, on| s.hide_system_drives = on,
                      cx,
                    )),
                ),
            ),
        )
    });
  });
}

fn setting_switch(
  id: &'static str,
  title: impl Into<String>,
  hint: impl Into<String>,
  on: bool,
  view: Entity<ImprintApp>,
  flip: fn(&mut Settings, bool),
  cx: &App,
) -> impl IntoElement {
  let title = title.into();
  let hint = hint.into();
  let g = glass(cx);
  gpui_component::h_flex()
    .id(id)
    .justify_between()
    .items_start()
    .gap_4()
    .px_4()
    .py_3()
    .hover(|s| s.bg(g.fill_hover))
    .child(
      v_flex()
        .gap_1()
        .child(div().child(title))
        .child(muted(cx, hint)),
    )
    .child(Switch::new(id).checked(on).on_click(move |checked, _, cx| {
      let on = *checked;
      view.update(cx, |this, cx| {
        flip(&mut this.settings, on);
        if id == "hide-system" {
          this.refresh_disks(cx);
        }
        cx.notify();
      });
    }))
}

fn locale_dropdown(current: Language, view: Entity<ImprintApp>, cx: &App) -> impl IntoElement {
  let rim = if cx.theme().is_dark() {
    cx.theme().accent.divide(0.50)
  } else {
    cx.theme().primary.divide(0.42)
  };
  Button::new("locale")
    .w_full()
    .outline()
    .rounded(ButtonRounded::Large)
    .icon(IconName::Globe)
    .label(current.native_name())
    .dropdown_caret(true)
    .bg(cx.theme().background)
    .border_color(rim)
    .text_color(cx.theme().foreground)
    .dropdown_menu(move |menu, _, _| {
      Language::ALL
        .into_iter()
        .fold(menu.min_w(px(280.)), |menu, lang| {
          menu.item(
            PopupMenuItem::new(lang.native_name())
              .checked(current == lang)
              .on_click({
                let view = view.clone();
                move |_, _, cx| {
                  view.update(cx, |this, cx| {
                    this.set_locale(LocalePref::Language(lang), cx);
                  });
                }
              }),
          )
        })
    })
}
