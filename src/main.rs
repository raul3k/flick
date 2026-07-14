use std::cell::RefCell;
use pipewire as pw;
use pw::{node::Node, proxy::Listener, types::ObjectType};
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Switch, Label, Orientation};
use gtk::glib;
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

                let is_source = obj.props.and_then(|d| d.get("media.class")) == Some("Audio/Source");

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
    if let Some(arg) = std::env::args().nth(1) {
        run_cli(&arg);
    } else {
        run_gui();
    }
}
