use gpui::{
  App, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
  StatefulInteractiveElement, Styled, Window, div, prelude::*, px,
};
use gpui_component::{
  ActiveTheme as _, Colorize as _, Disableable as _, Icon, IconName, Sizable as _,
  button::{Button, ButtonCustomVariant, ButtonRounded, ButtonVariants as _},
  h_flex,
  input::Input,
  menu::{DropdownMenu as _, PopupMenuItem},
  progress::ProgressCircle,
  select::Select,
  separator::Separator,
  spinner::Spinner,
  switch::Switch,
  v_flex,
};
use imprint_core::format_bytes;
use imprint_core::i18n::{t, tr};
use imprint_rpi::{InitFormat, OsItem, cached_path, filter_items};

use crate::app::ImprintApp;
use crate::rpi::{CatalogStatus, ChoiceSelect, DownloadStatus, RpiStep};
use crate::theme::{glass, raspberry_pi};
use crate::widgets::{bytes_progress, glass_surface, hover_fill, icon_badge, muted, section_label};

pub(crate) fn page(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let view = cx.entity();
  v_flex()
    .size_full()
    .min_h_0()
    .gap_2()
    .child(toolbar(app, cx))
    .child(step_panel(app, cx))
    .child(footer(app, view, cx))
}

pub(crate) fn download_panel(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let (received, total, name) = match &app.rpi.download {
    DownloadStatus::Running {
      received,
      total,
      name,
    } => (*received, *total, name.clone()),
    DownloadStatus::Idle => (0, None, String::new()),
  };
  let fraction = total
    .filter(|n| *n > 0)
    .map(|n| (received as f32 / n as f32).clamp(0.0, 1.0))
    .unwrap_or(0.0);
  let pct = (fraction * 100.0) as u32;
  let view = cx.entity();
  let ring = if cx.theme().is_dark() {
    cx.theme().accent
  } else {
    cx.theme().primary
  };
  v_flex().size_full().items_center().justify_center().child(
    glass_surface(
      v_flex()
        .w(px(420.))
        .max_w_full()
        .items_center()
        .gap_4()
        .px_8()
        .py_8(),
      cx,
    )
    .child(section_label(cx, t("rpi.downloading")))
    .child(
      ProgressCircle::new("rpi-download")
        .size(px(148.))
        .value(if total.is_some() { pct as f32 } else { 0.0 })
        .loading(total.is_none())
        .color(ring)
        .child(if total.is_none() {
          Spinner::new()
            .icon(Icon::new(IconName::LoaderCircle))
            .color(ring)
            .into_any_element()
        } else {
          div()
            .text_3xl()
            .font_weight(FontWeight::SEMIBOLD)
            .child(format!("{pct}%"))
            .into_any_element()
        }),
    )
    .child(
      div()
        .w_full()
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .text_center()
        .font_weight(FontWeight::MEDIUM)
        .truncate()
        .child(name),
    )
    .child(bytes_progress(
      cx,
      format_bytes(received),
      total
        .filter(|n| *n > 0)
        .map(format_bytes)
        .unwrap_or_else(|| "—".into()),
    ))
    .child(
      Button::new("rpi-download-cancel")
        .ghost()
        .rounded(ButtonRounded::Large)
        .label(t("progress.cancel"))
        .on_click(move |_, _, cx| {
          view.update(cx, |this, cx| {
            this
              .cancel
              .store(true, std::sync::atomic::Ordering::Relaxed);
            cx.notify();
          });
        }),
    ),
  )
}

fn toolbar(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  glass_surface(
    h_flex()
      .w_full()
      .items_center()
      .gap_2()
      .px_2()
      .py_1()
      .child(leave_button(cx))
      .child(step_rail(app, cx)),
    cx,
  )
  .w_full()
  .flex_shrink_0()
}

fn leave_button(cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let view = cx.entity();
  let raspberry = raspberry_pi(cx);
  let dark = cx.theme().is_dark();
  Button::new("rpi-leave")
    .small()
    .compact()
    .icon(IconName::ArrowLeft)
    .tooltip(t("rpi.back"))
    .rounded(ButtonRounded::Size(px(8.)))
    .custom(
      ButtonCustomVariant::new(cx)
        .color(raspberry)
        .hover(raspberry.divide(if dark { 0.34 } else { 0.16 }))
        .active(raspberry.divide(if dark { 0.46 } else { 0.24 }))
        .foreground(raspberry),
    )
    .bg(raspberry.divide(if dark { 0.28 } else { 0.10 }))
    .border_color(raspberry.divide(if dark { 0.70 } else { 0.45 }))
    .on_click(move |_, _, cx| {
      view.update(cx, |this, cx| this.leave_raspberry_pi(cx));
    })
}

fn step_rail(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let view = cx.entity();
  let current = app.rpi.step.index();
  let items = [
    (RpiStep::Device, t("rpi.step.device")),
    (RpiStep::Os, t("rpi.step.os")),
    (RpiStep::Config, t("rpi.step.config")),
    (RpiStep::Storage, t("rpi.step.storage")),
  ];
  let raspberry = raspberry_pi(cx);
  let hover = hover_fill(cx);
  let dark = cx.theme().is_dark();
  let mut children: Vec<gpui::AnyElement> = Vec::new();
  for (ix, (step, label)) in items.into_iter().enumerate() {
    if ix > 0 {
      let passed = current >= ix;
      children.push(
        div()
          .flex_1()
          .h(px(1.5))
          .rounded_full()
          .bg(if passed {
            raspberry.divide(0.55)
          } else {
            cx.theme().border
          })
          .into_any_element(),
      );
    }
    let selected = current == ix;
    let completed = current > ix;
    let openable = app.rpi_can_open_step(step);
    let view = view.clone();
    children.push(
      h_flex()
        .id(("rpi-step", ix))
        .flex_shrink_0()
        .items_center()
        .gap_1p5()
        .px_2()
        .py_1()
        .rounded_full()
        .when(selected, |d| {
          d.bg(raspberry.divide(if dark { 0.32 } else { 0.12 }))
        })
        .when(openable, |d| {
          d.cursor_pointer()
            .hover(move |s| if selected { s } else { s.bg(hover) })
            .on_click(move |_, _, cx| {
              view.update(cx, |this, cx| this.set_rpi_step(step, cx));
            })
        })
        .when(!openable, |d| d.opacity(0.45))
        .child(
          div()
            .flex()
            .items_center()
            .justify_center()
            .size(px(18.))
            .rounded_full()
            .bg(if selected || completed {
              raspberry
            } else {
              cx.theme().muted
            })
            .child(
              div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if selected || completed {
                  gpui::hsla(0., 0., 1., 1.)
                } else {
                  cx.theme().muted_foreground
                })
                .child(format!("{}", ix + 1)),
            ),
        )
        .child(
          div()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if selected {
              raspberry
            } else if completed {
              cx.theme().foreground
            } else {
              cx.theme().muted_foreground
            })
            .child(label),
        )
        .into_any_element(),
    );
  }
  h_flex()
    .flex_1()
    .min_w_0()
    .items_center()
    .gap_1()
    .children(children)
}

fn step_panel(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let body = step_body(app, cx);
  if app.rpi.step == RpiStep::Config {
    v_flex()
      .id("rpi-step-body")
      .flex_1()
      .min_h_0()
      .overflow_y_scroll()
      .child(body)
      .into_any_element()
  } else {
    glass_surface(v_flex().w_full().px_3().py_2().child(body), cx)
      .id("rpi-step-body")
      .flex_1()
      .min_h_0()
      .w_full()
      .overflow_y_scroll()
      .into_any_element()
  }
}

fn step_body(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  match app.rpi.step {
    RpiStep::Device => device_step(app, cx).into_any_element(),
    RpiStep::Os => os_step(app, cx).into_any_element(),
    RpiStep::Config => config_step(app, cx).into_any_element(),
    RpiStep::Storage => storage_step(app, cx).into_any_element(),
  }
}

fn device_step(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let view = cx.entity();
  v_flex().w_full().gap_1p5().child(match &app.rpi.catalog {
    CatalogStatus::Loading | CatalogStatus::Idle => {
      loading_row(cx, t("rpi.os.loading")).into_any_element()
    }
    CatalogStatus::Failed(err) => failed_row(err, cx).into_any_element(),
    CatalogStatus::Ready(catalog) => v_flex()
      .w_full()
      .gap_1p5()
      .children(
        catalog
          .imager
          .devices
          .iter()
          .enumerate()
          .map(|(ix, device)| {
            let selected = app.rpi.selected_device == Some(ix);
            let view = view.clone();
            list_row(
              cx,
              ("rpi-device", ix),
              IconName::Cpu,
              device.name.clone(),
              device.description.clone(),
              selected,
              move |_, _, cx| {
                view.update(cx, |this, cx| this.select_rpi_device(ix, cx));
              },
            )
          }),
      )
      .into_any_element(),
  })
}

fn os_step(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let view = cx.entity();
  let device = app.rpi.selected_device();
  let items = filter_items(app.rpi.current_os_list(), device);
  v_flex()
    .w_full()
    .gap_1p5()
    .when(!app.rpi.os_stack.is_empty(), |d| {
      d.child(
        Button::new("rpi-os-up")
          .ghost()
          .small()
          .compact()
          .icon(IconName::ChevronLeft)
          .label(t("rpi.os.back"))
          .on_click({
            let view = view.clone();
            move |_, _, cx| {
              view.update(cx, |this, cx| this.rpi_os_back(cx));
            }
          }),
      )
    })
    .child(match &app.rpi.catalog {
      CatalogStatus::Loading | CatalogStatus::Idle => {
        loading_row(cx, t("rpi.os.loading")).into_any_element()
      }
      CatalogStatus::Failed(err) => failed_row(err, cx).into_any_element(),
      CatalogStatus::Ready(_) if items.is_empty() => {
        empty_row(cx, t("rpi.os.empty")).into_any_element()
      }
      CatalogStatus::Ready(_) => v_flex()
        .w_full()
        .gap_1p5()
        .children(items.into_iter().map(|(ix, item)| {
          let selected = app.rpi.selected_os.as_ref().is_some_and(|os| {
            !os.is_local() && os.url.is_some() && os.url == item.url && os.name == item.name
          });
          os_row(ix, item, selected, view.clone(), cx)
        }))
        .into_any_element(),
    })
    .child(custom_os_row(app, view, cx))
}

fn os_row(
  ix: usize,
  item: &OsItem,
  selected: bool,
  view: gpui::Entity<ImprintApp>,
  cx: &App,
) -> impl IntoElement {
  let subtitle = if item.is_category() {
    item.description.clone()
  } else {
    let mut parts = Vec::new();
    if !item.release_date.is_empty() {
      parts.push(item.release_date.clone());
    }
    if item.image_download_size > 0 {
      parts.push(format_bytes(item.image_download_size));
    }
    if item.extract_size > 0 {
      parts.push(tr(
        "rpi.os.size",
        &[("size", &format_bytes(item.extract_size))],
      ));
    }
    if parts.is_empty() {
      item.description.clone()
    } else if item.description.is_empty() {
      parts.join(" · ")
    } else {
      format!("{} · {}", item.description, parts.join(" · "))
    }
  };
  let icon = if item.is_category() {
    IconName::Folder
  } else {
    IconName::Cpu
  };
  list_row(
    cx,
    ("rpi-os", ix),
    icon,
    item.name.clone(),
    subtitle,
    selected,
    move |_, _, cx| {
      view.update(cx, |this, cx| this.open_rpi_os_item(ix, cx));
    },
  )
  .when(item.is_category(), |d| {
    d.child(Icon::new(IconName::ChevronRight).text_color(cx.theme().muted_foreground))
  })
  .when(item.is_image() && cached_path(item).is_some(), |d| {
    d.child(
      div()
        .text_xs()
        .text_color(cx.theme().accent)
        .child(t("rpi.os.cached")),
    )
  })
}

fn custom_os_row(app: &ImprintApp, view: gpui::Entity<ImprintApp>, cx: &App) -> impl IntoElement {
  let selected = app.rpi.selected_os.as_ref().is_some_and(|os| os.is_local());
  let title = if selected {
    app
      .rpi
      .selected_os
      .as_ref()
      .map(|os| os.name.clone())
      .unwrap_or_else(|| t("rpi.os.custom"))
  } else {
    t("rpi.os.custom")
  };
  let subtitle = if selected {
    app
      .rpi
      .selected_os
      .as_ref()
      .and_then(|os| os.local_path())
      .map(|path| path.display().to_string())
      .unwrap_or_else(|| t("rpi.os.custom_hint"))
  } else {
    t("rpi.os.custom_hint")
  };
  list_row(
    cx,
    "rpi-os-custom",
    IconName::FolderOpen,
    title,
    subtitle,
    selected,
    {
      let view = view.clone();
      move |_, window, cx| {
        view.update(cx, |this, cx| this.pick_custom_os(window, cx));
      }
    },
  )
  .child(
    Button::new("rpi-os-custom-pick")
      .small()
      .rounded(ButtonRounded::Large)
      .label(t("image.select"))
      .on_click({
        let view = view.clone();
        move |_, window, cx| {
          cx.stop_propagation();
          view.update(cx, |this, cx| this.pick_custom_os(window, cx));
        }
      }),
  )
}

fn config_step(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let custom = app.rpi.selected_os.as_ref().is_some_and(|os| os.is_local());
  let init = app
    .rpi
    .selected_os
    .as_ref()
    .map(|os| os.init_format())
    .unwrap_or(InitFormat::None);
  let view = cx.entity();
  v_flex()
    .w_full()
    .gap_2()
    .pb_2()
    .when(custom, |d| {
      d.child(init_format_block(init, view.clone(), cx))
    })
    .when(!init.supports_customisation(), |d| {
      d.child(glass_surface(
        v_flex()
          .w_full()
          .gap_2()
          .px_4()
          .py_6()
          .items_center()
          .child(muted(cx, t("rpi.config.none_available"))),
        cx,
      ))
    })
    .when(init.supports_customisation(), |d| {
      d.child(config_block(
        cx,
        "rpi-cfg-hostname",
        t("rpi.config.hostname"),
        t("rpi.config.hostname_hint"),
        app.rpi.set_hostname,
        {
          let view = view.clone();
          move |on, cx| {
            view.update(cx, |this, cx| {
              this.rpi.set_hostname = on;
              cx.notify();
            });
          }
        },
        labeled_input(
          cx,
          t("rpi.config.hostname"),
          Input::new(&app.rpi.fields.hostname).cleanable(true),
        ),
      ))
      .child(config_block(
        cx,
        "rpi-cfg-user",
        t("rpi.config.user"),
        t("rpi.config.user_hint"),
        app.rpi.set_user,
        {
          let view = view.clone();
          move |on, cx| {
            view.update(cx, |this, cx| {
              this.rpi.set_user = on;
              cx.notify();
            });
          }
        },
        v_flex()
          .gap_2()
          .child(labeled_input(
            cx,
            t("rpi.config.username"),
            Input::new(&app.rpi.fields.username).cleanable(true),
          ))
          .child(labeled_input(
            cx,
            t("rpi.config.password"),
            Input::new(&app.rpi.fields.password).mask_toggle(),
          )),
      ))
      .child(config_block(
        cx,
        "rpi-cfg-wifi",
        t("rpi.config.wifi"),
        t("rpi.config.wifi_hint"),
        app.rpi.set_wifi,
        {
          let view = view.clone();
          move |on, cx| {
            view.update(cx, |this, cx| {
              this.rpi.set_wifi = on;
              cx.notify();
            });
          }
        },
        v_flex()
          .gap_2()
          .child(labeled_input(
            cx,
            t("rpi.config.ssid"),
            Input::new(&app.rpi.fields.wifi_ssid).cleanable(true),
          ))
          .child(labeled_input(
            cx,
            t("rpi.config.wifi_password"),
            Input::new(&app.rpi.fields.wifi_password).mask_toggle(),
          ))
          .child(labeled_choice(
            cx,
            t("rpi.config.wifi_country"),
            choice_dropdown(&app.rpi.fields.wifi_country, cx),
          )),
      ))
      .child(config_block(
        cx,
        "rpi-cfg-ssh",
        t("rpi.config.ssh"),
        t("rpi.config.ssh_hint"),
        app.rpi.set_ssh,
        {
          let view = view.clone();
          move |on, cx| {
            view.update(cx, |this, cx| {
              this.rpi.set_ssh = on;
              cx.notify();
            });
          }
        },
        labeled_input(
          cx,
          t("rpi.config.ssh_key"),
          Input::new(&app.rpi.fields.ssh_key).cleanable(true),
        ),
      ))
      .child(config_block(
        cx,
        "rpi-cfg-locale",
        t("rpi.config.locale"),
        t("rpi.config.locale_hint"),
        app.rpi.set_locale,
        {
          let view = view.clone();
          move |on, cx| {
            view.update(cx, |this, cx| {
              this.rpi.set_locale = on;
              cx.notify();
            });
          }
        },
        v_flex()
          .gap_2()
          .child(labeled_choice(
            cx,
            t("rpi.config.timezone"),
            choice_dropdown(&app.rpi.fields.timezone, cx),
          ))
          .child(labeled_choice(
            cx,
            t("rpi.config.keyboard"),
            choice_dropdown(&app.rpi.fields.keyboard, cx),
          )),
      ))
    })
    .into_any_element()
}

fn storage_step(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let view = cx.entity();
  let title = if app.selected.len() == 1 {
    app.selected_disks()[0].label()
  } else if app.selected.len() > 1 {
    tr("target.count", &[("n", &app.selected.len().to_string())])
  } else {
    t("target.none")
  };
  let subtitle = if let Some(disk) = app.selected_disks().first() {
    format!("{} · {}", disk.bus.as_str(), format_bytes(disk.size))
  } else {
    t("target.hint")
  };
  v_flex().w_full().gap_1p5().child(
    list_row(
      cx,
      "rpi-storage",
      IconName::HardDrive,
      title,
      subtitle,
      !app.selected.is_empty(),
      {
        let view = view.clone();
        move |_, window, cx| {
          view.update(cx, |this, cx| {
            if !this.flashing {
              this.open_drives(window, cx);
            }
          });
        }
      },
    )
    .child(
      Button::new("rpi-pick-drive")
        .small()
        .rounded(ButtonRounded::Large)
        .label(t("target.select"))
        .on_click({
          let view = view.clone();
          move |_, window, cx| {
            cx.stop_propagation();
            view.update(cx, |this, cx| this.open_drives(window, cx));
          }
        }),
    ),
  )
}

fn footer(
  app: &ImprintApp,
  view: gpui::Entity<ImprintApp>,
  cx: &mut Context<ImprintApp>,
) -> impl IntoElement {
  let can_write = app.can_rpi_write();
  let g = glass(cx);
  h_flex()
    .w_full()
    .flex_shrink_0()
    .items_center()
    .gap_2()
    .child(
      div()
        .flex_1()
        .min_w_0()
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(cx.theme().muted_foreground)
        .truncate()
        .child(step_hint(app)),
    )
    .child(
      Button::new("rpi-prev")
        .ghost()
        .small()
        .compact()
        .rounded(ButtonRounded::Large)
        .label(t("rpi.prev"))
        .disabled(app.rpi.step == RpiStep::Device)
        .on_click({
          let view = view.clone();
          move |_, _, cx| {
            view.update(cx, |this, cx| this.rpi_prev_step(cx));
          }
        }),
    )
    .child(if app.rpi.step == RpiStep::Storage {
      Button::new("rpi-write")
        .primary()
        .small()
        .compact()
        .rounded(ButtonRounded::Large)
        .label(t("rpi.write"))
        .disabled(!can_write)
        .on_click({
          let view = view.clone();
          move |_, _, cx| {
            view.update(cx, |this, cx| this.begin_rpi_write(cx));
          }
        })
        .into_any_element()
    } else {
      Button::new("rpi-next")
        .small()
        .compact()
        .custom(
          ButtonCustomVariant::new(cx)
            .color(g.fill)
            .hover(g.fill_hover)
            .foreground(if cx.theme().is_dark() {
              cx.theme().accent
            } else {
              cx.theme().primary
            }),
        )
        .rounded(ButtonRounded::Large)
        .label(t("rpi.next"))
        .disabled(!app.rpi_can_open_step(match app.rpi.step {
          RpiStep::Device => RpiStep::Os,
          RpiStep::Os => RpiStep::Config,
          RpiStep::Config => RpiStep::Storage,
          RpiStep::Storage => RpiStep::Storage,
        }))
        .on_click(move |_, _, cx| {
          view.update(cx, |this, cx| this.rpi_next_step(cx));
        })
        .into_any_element()
    })
}

fn step_hint(app: &ImprintApp) -> String {
  match app.rpi.step {
    RpiStep::Device => t("rpi.device.hint"),
    RpiStep::Os => t("rpi.os.hint"),
    RpiStep::Config => t("rpi.config.hint"),
    RpiStep::Storage => t("rpi.storage.hint"),
  }
}

fn init_format_block(
  current: InitFormat,
  view: gpui::Entity<ImprintApp>,
  cx: &App,
) -> impl IntoElement {
  glass_surface(
    v_flex()
      .w_full()
      .gap_2()
      .px_4()
      .py_3()
      .child(
        div()
          .font_weight(FontWeight::MEDIUM)
          .child(t("rpi.config.init")),
      )
      .child(muted(cx, t("rpi.config.init_hint")))
      .child(
        Button::new("rpi-init-format")
          .w_full()
          .outline()
          .rounded(ButtonRounded::Size(field_radius()))
          .dropdown_caret(true)
          .label(init_format_label(current))
          .bg(field_fill(cx))
          .border_color(field_rim(cx))
          .text_color(cx.theme().foreground)
          .dropdown_menu(move |menu, _, _| {
            InitFormat::ALL.into_iter().fold(
              menu.min_w(px(280.)).max_h(px(280.)).scrollable(true),
              |menu, format| {
                menu.item(
                  PopupMenuItem::new(init_format_label(format))
                    .checked(current == format)
                    .on_click({
                      let view = view.clone();
                      move |_, _, cx| {
                        view.update(cx, |this, cx| this.set_rpi_init_format(format, cx));
                      }
                    }),
                )
              },
            )
          }),
      ),
    cx,
  )
}

fn init_format_label(format: InitFormat) -> String {
  match format {
    InitFormat::None => t("rpi.config.init_none"),
    InitFormat::Systemd => t("rpi.config.init_systemd"),
    InitFormat::CloudInit => t("rpi.config.init_cloudinit"),
    InitFormat::CloudInitRpi => t("rpi.config.init_cloudinit_rpi"),
  }
}

fn config_block(
  cx: &App,
  id: &'static str,
  title: String,
  hint: String,
  on: bool,
  flip: impl Fn(bool, &mut App) + Clone + 'static,
  extra: impl IntoElement,
) -> impl IntoElement {
  let hover = hover_fill(cx);
  glass_surface(
    v_flex()
      .w_full()
      .flex_shrink_0()
      .child(
        h_flex()
          .id(id)
          .w_full()
          .items_start()
          .justify_between()
          .gap_4()
          .px_4()
          .py_2()
          .cursor_pointer()
          .hover(move |s| s.bg(hover))
          .on_click({
            let flip = flip.clone();
            move |_, _, cx| flip(!on, cx)
          })
          .child(
            v_flex()
              .flex_1()
              .min_w_0()
              .gap_0p5()
              .child(div().font_weight(FontWeight::MEDIUM).child(title))
              .child(muted(cx, hint)),
          )
          .child(
            Switch::new((id, 1usize))
              .checked(on)
              .on_click(move |checked, _, cx| {
                cx.stop_propagation();
                flip(*checked, cx);
              }),
          ),
      )
      .when(on, |d| {
        d.child(Separator::horizontal())
          .child(v_flex().w_full().gap_2().px_4().py_2().child(extra))
      }),
    cx,
  )
}

fn field_radius() -> gpui::Pixels {
  px(8.)
}

fn field_rim(cx: &App) -> gpui::Hsla {
  if cx.theme().is_dark() {
    cx.theme().accent.divide(0.62)
  } else {
    cx.theme().primary.divide(0.50)
  }
}

fn field_fill(cx: &App) -> gpui::Hsla {
  if cx.theme().is_dark() {
    cx.theme().background
  } else {
    cx.theme().popover
  }
}

fn labeled_input(cx: &App, label: String, input: Input) -> impl IntoElement {
  labeled_choice(
    cx,
    label,
    div().w_full().flex_shrink_0().h(px(36.)).child(
      input
        .w_full()
        .rounded(field_radius())
        .bg(field_fill(cx))
        .border_color(field_rim(cx)),
    ),
  )
}

fn labeled_choice(cx: &App, label: String, control: impl IntoElement) -> impl IntoElement {
  v_flex()
    .w_full()
    .flex_shrink_0()
    .gap_1()
    .child(
      div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(label),
    )
    .child(control)
}

fn choice_dropdown(state: &gpui::Entity<ChoiceSelect>, cx: &App) -> impl IntoElement {
  Select::new(state)
    .w_full()
    .menu_max_h(px(280.))
    .rounded(field_radius())
    .bg(field_fill(cx))
    .border_color(field_rim(cx))
    .text_color(cx.theme().foreground)
}

fn list_row(
  cx: &App,
  id: impl Into<gpui::ElementId>,
  icon: IconName,
  title: String,
  subtitle: String,
  selected: bool,
  on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
  let g = glass(cx);
  let hover = hover_fill(cx);
  h_flex()
    .id(id)
    .w_full()
    .flex_shrink_0()
    .items_center()
    .gap_2()
    .px_2()
    .py_1p5()
    .rounded(cx.theme().radius)
    .border_1()
    .border_color(if selected {
      cx.theme().list_active_border
    } else {
      gpui::Hsla {
        a: g.border.a * 0.55,
        ..g.border
      }
    })
    .bg(if selected {
      cx.theme().list_active
    } else {
      cx.theme().transparent
    })
    .cursor_pointer()
    .hover(move |s| if selected { s } else { s.bg(hover) })
    .on_click(on_click)
    .child(icon_badge(cx, icon, selected, px(32.)))
    .child(
      v_flex()
        .flex_1()
        .gap_0p5()
        .min_w_0()
        .child(
          div()
            .font_weight(FontWeight::MEDIUM)
            .truncate()
            .child(title),
        )
        .child(
          div()
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(cx.theme().muted_foreground)
            .truncate()
            .child(subtitle),
        ),
    )
}

fn loading_row(cx: &App, text: String) -> impl IntoElement {
  h_flex()
    .w_full()
    .items_center()
    .justify_center()
    .gap_2()
    .py_6()
    .child(
      Spinner::new()
        .icon(Icon::new(IconName::LoaderCircle))
        .color(cx.theme().accent),
    )
    .child(muted(cx, text))
}

fn empty_row(cx: &App, text: String) -> impl IntoElement {
  v_flex()
    .w_full()
    .items_center()
    .py_6()
    .child(muted(cx, text))
}

fn failed_row(err: &str, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let view = cx.entity();
  v_flex()
    .w_full()
    .items_center()
    .gap_2()
    .py_6()
    .child(muted(cx, err.to_string()))
    .child(
      Button::new("rpi-retry")
        .small()
        .label(t("rpi.os.retry"))
        .on_click(move |_, _, cx| {
          view.update(cx, |this, cx| this.fetch_rpi_catalog(cx));
        }),
    )
}
