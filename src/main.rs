use gtk::cairo;
use gtk::glib;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Label, Orientation, Switch};
use ksni::blocking::TrayMethods;
use pipewire as pw;
use pw::{node::Node, proxy::Listener, types::ObjectType};
use std::cell::RefCell;
use std::process::Command;

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
        ("Microfone: mutado", "off")
    } else {
        ("Microfone: ligado", "on")
    };

    label.set_text(text);
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
        ctx.arc(12.0, 12.0, 9.5, 0.0, 2.0 * PI);
        ctx.fill().expect("fill dot");
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
            let (mut r, mut g, mut b) = ((px >> 16) & 0xff, (px >> 8) & 0xff, px & 0xff);
            if a > 0 {
                r = (r * 255 / a).min(255);
                g = (g * 255 / a).min(255);
                b = (b * 255 / a).min(255);
            }
            let o = (y * width as usize + x) * 4;
            out[o] = a as u8;
            out[o + 1] = r as u8;
            out[o + 2] = g as u8;
            out[o + 3] = b as u8;
        }
    }

    ksni::Icon {
        width,
        height,
        data: out,
    }
}

/// Opens (or focuses) the Flick window by relaunching the binary with no args.
fn open_window() {
    if let Ok(exe) = std::env::current_exe() {
        Command::new(exe).spawn().ok();
    }
}

struct MicTray {
    muted: bool,
    icon_on: ksni::Icon,
    icon_off: ksni::Icon,
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
                "Microfone: mutado".into()
            } else {
                "Microfone: ligado".into()
            },
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        open_window();
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        vec![
            StandardItem {
                label: "Abrir Flick".into(),
                activate: Box::new(|_| open_window()),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Sair".into(),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn run_tray() {
    let tray = MicTray {
        muted: is_muted(),
        icon_on: make_icon(true),
        icon_off: make_icon(false),
    };
    let handle = tray.spawn().expect("could not register tray icon");

    // pipewire -> keep the tray in sync with the real mic state
    let (tx, rx) = async_channel::unbounded::<()>();
    spawn_pipewire_listener(tx);

    while rx.recv_blocking().is_ok() {
        let muted = is_muted();
        if handle.update(|t: &mut MicTray| t.muted = muted).is_none() {
            break; // tray service was shut down
        }
    }
}

fn run_gui() {
    let app = Application::builder()
        .application_id("io.github.raul3k.flick")
        .build();

    app.connect_activate(|app| {
        let css = gtk::CssProvider::new();
        css.load_from_data(
            r#"
            .indicator { border-radius: 8px; min-width: 16px; min-height: 16px; }
            .indicator.on { background-color: #2ecc71; }
            .indicator.off { background-color: #e74c3c; }
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

        // pipewire -> channel -> UI
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
            }
        });

        let row = gtk::Box::new(Orientation::Horizontal, 8);
        row.append(&indicator);
        row.append(&label);
        row.append(&switch);

        let container = gtk::Box::new(Orientation::Vertical, 12);
        container.set_margin_top(16);
        container.set_margin_bottom(16);
        container.set_margin_start(16);
        container.set_margin_end(16);
        container.append(&row);

        let window = ApplicationWindow::builder()
            .application(app)
            .title("Mic Flick")
            .default_width(300)
            .default_height(150)
            .child(&container)
            .build();

        window.present();
    });

    app.run();
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
                println!("Microfone: mutado");
            } else {
                println!("Microfone: ligado");
            }
        }
    }
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("tray") => run_tray(),
        Some(arg) => run_cli(arg),
        None => run_gui(),
    }
}
