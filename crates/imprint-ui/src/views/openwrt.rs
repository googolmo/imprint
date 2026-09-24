use crate::app::ImprintApp;
use crate::widgets::{glass_surface, muted, section_label};
use gpui::{Context, FontWeight, IntoElement, ParentElement, Styled, div, prelude::*, px};
use gpui_component::scroll::ScrollableElement as _;
use gpui_component::{
  ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
  button::{Button, ButtonRounded, ButtonVariants as _},
  h_flex,
  input::Input,
  switch::Switch,
  v_flex,
};
use gpui_kit::{component as gpui_component, gpui};

pub(crate) fn page(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let view = cx.entity();
  let back = view.clone();
  v_flex()
    .size_full()
    .min_h_0()
    .gap_2()
    .child(glass_surface(
      h_flex()
        .w_full()
        .items_center()
        .justify_between()
        .px_3()
        .py_2()
        .child(
          h_flex()
            .items_center()
            .gap_2()
            .child(
              Icon::new(IconName::HardDrive)
                .size(px(18.))
                .text_color(cx.theme().primary),
            )
            .child(div().font_weight(FontWeight::SEMIBOLD).child("OpenWrt")),
        )
        .child(
          Button::new("ow-back")
            .ghost()
            .small()
            .rounded(ButtonRounded::Large)
            .label("Back")
            .on_click(move |_, _, cx| {
              back.update(cx, |x, cx| x.leave_openwrt(cx));
            }),
        ),
      cx,
    ))
    .child(
      v_flex()
        .flex_1()
        .min_h_0()
        .overflow_y_scrollbar()
        .gap_3()
        .py_2()
        .child(image_card(app, cx))
        .child(settings_card(app, cx))
        .child(storage_card(app, cx)),
    )
    .child(
      h_flex().w_full().justify_end().child(
        Button::new("ow-write")
          .primary()
          .rounded(ButtonRounded::Large)
          .label("Write image")
          .disabled(app.image.is_none() || app.selected.is_empty())
          .on_click(move |_, _, cx| {
            view.update(cx, |x, cx| x.begin_openwrt_write(cx));
          }),
      ),
    )
}
fn image_card(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let view = cx.entity();
  glass_surface(
    v_flex()
      .gap_2()
      .px_4()
      .py_4()
      .child(section_label(cx, "1 · LOCAL IMAGE"))
      .child(muted(
        cx,
        "Choose a local OpenWrt image. Nothing is downloaded.",
      ))
      .child(
        Button::new("ow-image")
          .large()
          .label(
            app
              .openwrt
              .path
              .as_ref()
              .map(|p| p.display().to_string())
              .unwrap_or_else(|| "Choose .img, .img.gz, or .img.xz".into()),
          )
          .on_click(move |_, window, cx| {
            view.update(cx, |x, cx| x.pick_openwrt_image(window, cx));
          }),
      ),
    cx,
  )
}
fn settings_card(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let f = &app.openwrt.fields;
  let view = cx.entity();
  glass_surface(
    v_flex()
      .gap_3()
      .px_4()
      .py_4()
      .child(section_label(cx, "2 · FIRST-BOOT SETTINGS"))
      .child(row("Hostname", Input::new(&f.hostname).cleanable(true)))
      .child(row("Root password", Input::new(&f.password).mask_toggle()))
      .child(row("SSH public keys", Input::new(&f.keys).cleanable(true)))
      .child(
        h_flex()
          .gap_2()
          .child(Switch::new("ow-wifi").checked(app.openwrt.wifi).on_click({
            let view = view.clone();
            move |_, _, cx| {
              view.update(cx, |x, cx| {
                x.openwrt.wifi = !x.openwrt.wifi;
                cx.notify();
              });
            }
          }))
          .child("Create a secured Wi-Fi access point"),
      )
      .when(app.openwrt.wifi, |d| {
        d.child(row("Wi-Fi SSID", Input::new(&f.ssid)))
          .child(row(
            "Wi-Fi password",
            Input::new(&f.wifi_password).mask_toggle(),
          ))
          .child(row("Country code", Input::new(&f.country)))
      })
      .child(
        div()
          .pt_2()
          .font_weight(FontWeight::SEMIBOLD)
          .child("LAN & DHCP"),
      )
      .child(row("LAN address", Input::new(&f.lan_ip)))
      .child(row("Prefix length", Input::new(&f.prefix)))
      .child(
        h_flex()
          .gap_2()
          .child(
            Switch::new("ow-dhcp")
              .checked(app.openwrt.dhcp)
              .on_click(move |_, _, cx| {
                view.update(cx, |x, cx| {
                  x.openwrt.dhcp = !x.openwrt.dhcp;
                  cx.notify();
                });
              }),
          )
          .child("Enable LAN DHCP service"),
      )
      .when(app.openwrt.dhcp, |d| {
        d.child(row("Pool start", Input::new(&f.dhcp_start)))
          .child(row("Pool size", Input::new(&f.dhcp_limit)))
          .child(row("Lease time", Input::new(&f.leasetime)))
      }),
    cx,
  )
}
fn storage_card(app: &ImprintApp, cx: &mut Context<ImprintApp>) -> impl IntoElement {
  let view = cx.entity();
  glass_surface(
    v_flex()
      .gap_2()
      .px_4()
      .py_4()
      .child(section_label(cx, "3 · STORAGE"))
      .child(muted(
        cx,
        if app.selected.is_empty() {
          "No target drive selected"
        } else {
          "Target drive selected"
        },
      ))
      .child(
        Button::new("ow-drives")
          .label("Select target drives")
          .on_click(move |_, window, cx| {
            view.update(cx, |x, cx| x.open_drives(window, cx));
          }),
      ),
    cx,
  )
}
fn row<E: IntoElement>(label: &str, input: E) -> impl IntoElement {
  h_flex()
    .items_center()
    .gap_3()
    .child(
      div()
        .w(px(130.))
        .flex_shrink_0()
        .text_sm()
        .child(label.to_string()),
    )
    .child(div().flex_1().child(input))
}
