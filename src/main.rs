mod config;
mod i18n;

use config::Config;
use gtk::cairo;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Label, Orientation, Switch};
use ksni::blocking::TrayMethods;
use pipewire as pw;
use pw::{node::Node, proxy::Listener, types::ObjectType};
use rust_i18n::t;
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

rust_i18n::i18n!("locales", fallback = "en");

/// What the tray thread can ask the GTK main loop to do. Tray callbacks run on
/// the D-Bus thread and must never touch widgets themselves.
enum UiMsg {
    Show,
    Refresh,
}

enum Action {
    On,
    Off,
    Toggle,
    Status,
}

fn spawn_pipewire_listener(tx: async_channel::Sender<()>) {
    std::thread::spawn(|| {
        pw::init();

        let main_loop = pw::main_loop::MainLoopRc::new(None).expect("main loop");
        let context = pw::context::ContextRc::new(&main_loop, None).expect("context");
        let core = context.connect_rc(None).expect("core");
        let registry = core.get_registry_rc().expect("registry");
        let registry_weak = registry.downgrade();

        // proxies and listeners must stay alive, otherwise events stop arriving
        let kept: RefCell<Vec<(Node, Box<dyn Listener>)>> = RefCell::new(Vec::new());

        let _reg_listener = registry
            .add_listener_local()
            .global(move |obj| {
                if obj.type_ != ObjectType::Node {
                    return;
                }

                let is_source =
                    obj.props.and_then(|d| d.get("media.class")) == Some("Audio/Source");

                if !is_source {
                    return;
                }

                if let Some(registry) = registry_weak.upgrade() {
                    let node: Node = registry.bind(obj).expect("bind node");
                    let tx = tx.clone();
                    let listener = node
                        .add_listener_local()
                        .info(move |_info| {
                            tx.send_blocking(()).ok();
                        })
                        .register();
                    kept.borrow_mut().push((node, Box::new(listener)));
                }
            })
            .register();
        main_loop.run();
    });
}

fn refresh(muted: bool, indicator: &gtk::Box, label: &Label) {
    let (text, color) = if muted {
        (t!("status.muted"), "off")
    } else {
        (t!("status.live"), "on")
    };

    label.set_text(&text);
    indicator.set_css_classes(&["indicator", color]);
}

fn set_mute(value: &str) {
    Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SOURCE@", value])
        .status()
        .expect("could not run wpctl");
}

fn is_muted() -> bool {
    let output = Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SOURCE@"])
        .output()
        .expect("could not run wpctl");

    let text = String::from_utf8_lossy(&output.stdout);
    text.contains("[MUTED]")
}

/// Draws the tray icon: a colored dot ("farol"), green when active and red
/// when muted. Deliberately NOT a microphone glyph, because GNOME already
/// shows one to signal that an app is recording. Returns it as an ARGB32
/// pixmap in network byte order, the format `ksni::Icon` expects.
fn make_icon(active: bool) -> ksni::Icon {
    // A square icon holding one big dot, so the panel renders it at bar size
    // like any other tray icon (battery, volume) instead of shrinking a wide
    // bitmap. Drawn in a 24x24 logical box at SCALE for a crisp downscale.
    use std::f64::consts::PI;
    const SCALE: i32 = 4;
    let width = 24 * SCALE;
    let height = 24 * SCALE;

    let mut surface =
        cairo::ImageSurface::create(cairo::Format::ARgb32, width, height).expect("cairo surface");
    {
        let ctx = cairo::Context::new(&surface).expect("cairo context");
        ctx.scale(SCALE as f64, SCALE as f64); // draw in logical 24x24 coords

        let (r, g, b) = if active {
            (0.18, 0.80, 0.44) // green
        } else {
            (0.91, 0.30, 0.24) // red
        };
        ctx.set_source_rgb(r, g, b);

        // rounded square, the brand tile, filling the whole canvas so the
        // panel has nothing to shrink away
        let (x, y, w, h, radius) = (0.5, 0.5, 23.0, 23.0, 5.5);
        ctx.new_sub_path();
        ctx.arc(x + w - radius, y + radius, radius, -0.5 * PI, 0.0);
        ctx.arc(x + w - radius, y + h - radius, radius, 0.0, 0.5 * PI);
        ctx.arc(x + radius, y + h - radius, radius, 0.5 * PI, PI);
        ctx.arc(x + radius, y + radius, radius, PI, 1.5 * PI);
        ctx.close_path();
        ctx.fill().expect("fill tile");

        // punch the "F" out of the tile so the panel shows through it, which
        // keeps the letter readable on both light and dark bars
        ctx.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
        ctx.set_font_size(22.0);
        let ext = ctx.text_extents("F").expect("text extents");
        ctx.move_to(
            x + (w - ext.width()) / 2.0 - ext.x_bearing(),
            y + (h - ext.height()) / 2.0 - ext.y_bearing(),
        );
        // fill plus stroke: cairo's toy font API tops out at Bold, so the
        // glyph is outlined as well to get a heavier weight than the font ships
        ctx.text_path("F");
        ctx.set_line_width(1.4);
        ctx.set_line_join(cairo::LineJoin::Round);
        ctx.set_operator(cairo::Operator::Clear);
        ctx.fill_preserve().expect("punch letter");
        ctx.stroke().expect("thicken letter");

        // Muted also gets a diagonal cut, so the state survives without color.
        // Green and red collapse under the common red-green deficiencies
        // (measured: only 40% apart in protanopia, and 1.8:1 in luminance),
        // so color alone would leave those users unable to read the state.
        if !active {
            ctx.set_line_width(3.0);
            ctx.move_to(4.0, 20.0);
            ctx.line_to(20.0, 4.0);
            ctx.stroke().expect("cut slash");
        }
        ctx.set_operator(cairo::Operator::Over);
    }
    surface.flush();

    let stride = surface.stride() as usize;
    let data = surface.data().expect("surface data");

    // cairo stores premultiplied native-endian ARGB32; ksni wants straight
    // (non-premultiplied) ARGB32 in network byte order (bytes A, R, G, B).
    let mut out = vec![0u8; (width * height * 4) as usize];
    for y in 0..height as usize {
        for x in 0..width as usize {
            let i = y * stride + x * 4;
            let px = u32::from_ne_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
            let a = (px >> 24) & 0xff;
            // undo cairo's premultiplication; a == 0 is fully transparent, so
            // the color channels carry nothing and stay at zero
            let straight = |c: u32| (c * 255).checked_div(a).unwrap_or(0).min(255) as u8;

            let o = (y * width as usize + x) * 4;
            out[o] = a as u8;
            out[o + 1] = straight((px >> 16) & 0xff);
            out[o + 2] = straight((px >> 8) & 0xff);
            out[o + 3] = straight(px & 0xff);
        }
    }

    ksni::Icon {
        width,
        height,
        data: out,
    }
}

struct MicTray {
    muted: bool,
    icon_on: ksni::Icon,
    icon_off: ksni::Icon,
    /// Current language setting: `auto`, or a code from [`i18n::LANGUAGES`].
    language: String,
    ui: async_channel::Sender<UiMsg>,
}

/// Switches the active language and remembers the choice. Both the tray menu
/// and the window menu funnel through here so they stay consistent.
fn apply_language(setting: &str) {
    i18n::apply(setting);
    Config {
        language: setting.to_string(),
    }
    .save();
}

impl MicTray {
    fn set_language(&mut self, index: usize) {
        self.language = i18n::setting_at(index);
        apply_language(&self.language);
        // the tray relabels itself once this callback returns, but the window
        // is on the other thread and has to be told
        self.ui.try_send(UiMsg::Refresh).ok();
    }
}

/// Builds the window's menu (the ☰ button). Mirrors the tray menu, minus
/// "Open Flick" which makes no sense from inside the window. Rebuilt on every
/// language change so its own labels follow the chosen locale.
fn build_window_menu() -> gio::Menu {
    let languages = gio::Menu::new();
    languages.append(
        Some(t!("tray.language_auto").as_ref()),
        Some(&format!("win.language::{}", config::AUTO)),
    );
    for (code, name) in i18n::LANGUAGES {
        languages.append(Some(name), Some(&format!("win.language::{code}")));
    }

    let preferences = gio::Menu::new();
    preferences.append_submenu(Some(t!("tray.language").as_ref()), &languages);

    // "File -> Quit" and "Preferences -> Language": every top-level entry of a
    // menu bar must be a menu, not a bare action, or GTK refuses to render it
    let file = gio::Menu::new();
    file.append(Some(t!("tray.quit").as_ref()), Some("win.quit"));

    let menu = gio::Menu::new();
    menu.append_submenu(Some(t!("menu.file").as_ref()), &file);
    menu.append_submenu(Some(t!("tray.preferences").as_ref()), &preferences);

    menu
}

impl ksni::Tray for MicTray {
    fn id(&self) -> String {
        "io.github.raul3k.flick".into()
    }

    fn title(&self) -> String {
        "Mic".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let icon = if self.muted {
            &self.icon_off
        } else {
            &self.icon_on
        };
        vec![icon.clone()]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Mic".into(),
            description: if self.muted {
                t!("status.muted").into_owned()
            } else {
                t!("status.live").into_owned()
            },
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.ui.try_send(UiMsg::Show).ok();
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{RadioGroup, RadioItem, StandardItem, SubMenu};

        let mut languages = vec![RadioItem {
            label: t!("tray.language_auto").into_owned(),
            ..Default::default()
        }];
        languages.extend(i18n::LANGUAGES.iter().map(|(_, name)| RadioItem {
            label: (*name).to_string(),
            ..Default::default()
        }));

        vec![
            StandardItem {
                label: t!("tray.open").into_owned(),
                activate: Box::new(|tray: &mut MicTray| {
                    tray.ui.try_send(UiMsg::Show).ok();
                }),
                ..Default::default()
            }
            .into(),
            SubMenu {
                label: t!("tray.preferences").into_owned(),
                submenu: vec![
                    SubMenu {
                        label: t!("tray.language").into_owned(),
                        submenu: vec![
                            RadioGroup {
                                selected: i18n::menu_index(&self.language),
                                select: Box::new(|tray: &mut MicTray, index| {
                                    tray.set_language(index)
                                }),
                                options: languages,
                            }
                            .into(),
                        ],
                        ..Default::default()
                    }
                    .into(),
                ],
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: t!("tray.quit").into_owned(),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Runs the tray indicator together with the window. `show_window` decides
/// whether the window starts visible (`flick`) or stays hidden until asked for
/// from the tray (`flick tray`, which is what autostart runs).
fn run_app(show_window: bool, config: Config) {
    let app = Application::builder()
        .application_id("io.github.raul3k.flick")
        .build();

    // Built on the first activation only. Later activations just present it,
    // so running `flick` again reaches the instance already in the tray
    // instead of stacking a second window and a second icon.
    let existing: Rc<RefCell<Option<ApplicationWindow>>> = Rc::new(RefCell::new(None));

    app.connect_activate(move |app| {
        if let Some(window) = existing.borrow().as_ref() {
            window.present();
            return;
        }
        let css = gtk::CssProvider::new();
        css.load_from_data(
            r#"
            .indicator { border-radius: 8px; min-width: 16px; min-height: 16px; }
            .indicator.on { background-color: #2ecc71; }
            .indicator.off { background-color: #e74c3c; }

            /* make the menu bar read as a bar, not loose text, before hover */
            menubar {
                background-color: @headerbar_bg_color;
                border-bottom: 1px solid @borders;
                padding: 2px 4px;
            }
            menubar > item {
                padding: 4px 12px;
                border-radius: 5px;
            }
            menubar > item:hover { background-color: alpha(currentColor, 0.1); }
            "#,
        );
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().expect("no display"),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let indicator = gtk::Box::new(Orientation::Horizontal, 0);
        indicator.set_size_request(16, 16);
        indicator.set_halign(gtk::Align::Center);
        indicator.set_valign(gtk::Align::Center);
        indicator.add_css_class("indicator");

        let label = Label::new(None);

        let switch = Switch::new();
        switch.set_halign(gtk::Align::End);
        switch.set_valign(gtk::Align::Center);
        switch.set_hexpand(true);

        let muted = is_muted();
        switch.set_active(!muted);
        refresh(muted, &indicator, &label);

        let indicator_clone = indicator.clone();
        let label_clone = label.clone();
        switch.connect_state_set(move |_sw, active| {
            set_mute(if active { "0" } else { "1" });
            refresh(!active, &indicator_clone, &label_clone);
            glib::Propagation::Proceed
        });

        // tray indicator, in this same process
        let (ui_tx, ui_rx) = async_channel::unbounded::<UiMsg>();
        let tray = MicTray {
            muted,
            icon_on: make_icon(true),
            icon_off: make_icon(false),
            language: config.language.clone(),
            ui: ui_tx,
        };
        let tray_handle = tray.spawn().expect("could not register tray icon");
        let tray_handle_lang = tray_handle.clone();

        // pipewire -> channel -> UI + tray
        let (tx, rx) = async_channel::unbounded::<()>();
        spawn_pipewire_listener(tx);

        let indicator_evt = indicator.clone();
        let label_evt = label.clone();
        let switch_evt = switch.clone();
        glib::spawn_future_local(async move {
            while rx.recv().await.is_ok() {
                let muted = is_muted();
                refresh(muted, &indicator_evt, &label_evt);
                switch_evt.set_active(!muted);
                tray_handle.update(|t: &mut MicTray| t.muted = muted);
            }
        });

        let row = gtk::Box::new(Orientation::Horizontal, 8);
        row.append(&indicator);
        row.append(&label);
        row.append(&switch);

        let content = gtk::Box::new(Orientation::Vertical, 12);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&row);

        // classic top menu bar (File-menu style), not a GNOME header button
        let menubar = gtk::PopoverMenuBar::from_model(Some(&build_window_menu()));

        let root = gtk::Box::new(Orientation::Vertical, 0);
        root.append(&menubar);
        root.append(&content);

        let window = ApplicationWindow::builder()
            .application(app)
            .title("Mic Flick")
            .default_width(300)
            .default_height(160)
            .child(&root)
            .build();

        // closing hides the window instead of destroying it, otherwise the
        // last window going away would take the tray indicator down with it
        window.connect_close_request(|window| {
            window.set_visible(false);
            glib::Propagation::Stop
        });

        // the menu's language radio; changing it here mirrors to the tray
        let language_action = gio::SimpleAction::new_stateful(
            "language",
            Some(glib::VariantTy::STRING),
            &config.language.to_variant(),
        );
        {
            let indicator = indicator.clone();
            let label = label.clone();
            let menubar = menubar.clone();
            language_action.connect_change_state(move |action, value| {
                let Some(value) = value else { return };
                let setting = value.str().unwrap_or(config::AUTO).to_string();
                action.set_state(value);
                apply_language(&setting);
                refresh(is_muted(), &indicator, &label);
                menubar.set_menu_model(Some(&build_window_menu()));
                tray_handle_lang.update(move |t: &mut MicTray| t.language = setting.clone());
            });
        }
        window.add_action(&language_action);

        let quit_action = gio::SimpleAction::new("quit", None);
        quit_action.connect_activate(|_, _| std::process::exit(0));
        window.add_action(&quit_action);

        let window_evt = window.clone();
        let indicator_ui = indicator.clone();
        let label_ui = label.clone();
        let menubar_ui = menubar.clone();
        let language_ui = language_action.clone();
        glib::spawn_future_local(async move {
            while let Ok(msg) = ui_rx.recv().await {
                match msg {
                    UiMsg::Show => window_evt.present(),
                    // the tray changed the language: mirror the radio, rebuild
                    // the menu labels, and re-translate the visible text
                    UiMsg::Refresh => {
                        let setting = Config::load().language;
                        language_ui.set_state(&setting.to_variant());
                        menubar_ui.set_menu_model(Some(&build_window_menu()));
                        refresh(is_muted(), &indicator_ui, &label_ui);
                    }
                }
            }
        });

        *existing.borrow_mut() = Some(window.clone());
        if show_window {
            window.present();
        }
    });

    // our own argv parsing already ran, so hand GTK an empty one: otherwise it
    // treats `tray` as a file to open, never fires `activate`, and exits
    app.run_with_args::<&str>(&[]);
}

fn run_cli(arg: &str) {
    let action = match arg {
        "on" => Action::On,
        "off" => Action::Off,
        "toggle" => Action::Toggle,
        "status" => Action::Status,
        other => panic!("unknown action: {other}"),
    };

    match action {
        Action::On => set_mute("0"),
        Action::Off => set_mute("1"),
        Action::Toggle => set_mute("toggle"),
        Action::Status => {
            if is_muted() {
                println!("{}", t!("status.muted"));
            } else {
                println!("{}", t!("status.live"));
            }
        }
    }
}

fn main() {
    let config = Config::load();
    i18n::apply(&config.language);

    match std::env::args().nth(1).as_deref() {
        Some("tray") => run_app(false, config),
        Some(arg) => run_cli(arg),
        None => run_app(true, config),
    }
}
