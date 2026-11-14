//! Small custom widgets: Lucide glyphs, the stage that positions the panel, an animated-height
//! container and the canvas behind result lists that slides the selection light.

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, graphene, gsk};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::time::Instant;

use super::glyph_paths::GLYPH_PATHS;

fn ease_out(t: f64) -> f64 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn animations_enabled() -> bool {
    gtk::Settings::default().is_none_or(|s| s.is_gtk_enable_animations())
}

// ── Glyph ──────────────────────────────────────────────────────────────────

thread_local! {
    static PATHS: RefCell<HashMap<&'static str, gsk::Path>> = RefCell::new(HashMap::new());
}

fn path_for(name: &'static str) -> Option<gsk::Path> {
    PATHS.with(|cache| {
        if let Some(p) = cache.borrow().get(name) {
            return Some(p.clone());
        }
        let data = GLYPH_PATHS.iter().find(|(n, _)| *n == name)?.1;
        let builder = gsk::PathBuilder::new();
        for part in data.split('|') {
            match gsk::Path::parse(part) {
                Ok(p) => builder.add_path(&p),
                Err(_) => eprintln!("[glyph] bad path in {name}: {part}"),
            }
        }
        let path = builder.to_path();
        cache.borrow_mut().insert(name, path.clone());
        Some(path)
    })
}

mod glyph_imp {
    use super::*;

    pub struct Glyph {
        pub name: Cell<&'static str>,
        pub size: Cell<i32>,
        pub stroke: Cell<f32>,
    }

    impl Default for Glyph {
        fn default() -> Self {
            Self { name: Cell::new("sparkles"), size: Cell::new(16), stroke: Cell::new(2.0) }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Glyph {
        const NAME: &'static str = "LumenGlyph";
        type Type = super::Glyph;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("glyph");
        }
    }

    impl ObjectImpl for Glyph {}

    impl WidgetImpl for Glyph {
        fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            let s = self.size.get();
            (s, s, -1, -1)
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let widget = self.obj();
            let Some(path) = path_for(self.name.get()) else { return };
            let (w, h) = (widget.width() as f32, widget.height() as f32);
            let size = w.min(h);
            if size <= 0.0 {
                return;
            }
            let scale = size / 24.0;
            snapshot.save();
            snapshot.translate(&graphene::Point::new((w - size) / 2.0, (h - size) / 2.0));
            snapshot.scale(scale, scale);
            let stroke = gsk::Stroke::new(self.stroke.get());
            stroke.set_line_cap(gsk::LineCap::Round);
            stroke.set_line_join(gsk::LineJoin::Round);
            snapshot.append_stroke(&path, &stroke, &widget.color());
            snapshot.restore();
        }
    }
}

glib::wrapper! {
    /// A Lucide line icon drawn in the widget's CSS colour.
    pub struct Glyph(ObjectSubclass<glyph_imp::Glyph>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Glyph {
    pub fn new(name: &'static str, size: i32) -> Self {
        let g: Glyph = glib::Object::new();
        g.imp().name.set(name);
        g.imp().size.set(size);
        g.add_css_class("glyph");
        g.set_can_target(false);
        g
    }

    pub fn with_stroke(self, width: f32) -> Self {
        self.imp().stroke.set(width);
        self
    }

    pub fn set_name(&self, name: &'static str) {
        if self.imp().name.get() != name {
            self.imp().name.set(name);
            self.queue_draw();
        }
    }
}

// ── Stage ──────────────────────────────────────────────────────────────────

/// Where the panel sits inside the window (logical pixels).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PanelRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

pub const PANEL_WIDTH: i32 = 780;
pub const PANEL_RADIUS: i32 = 26;

mod stage_imp {
    use super::*;

    #[derive(Default)]
    pub struct Stage {
        pub panel: RefCell<Option<gtk::Widget>>,
        pub rect: Cell<PanelRect>,
        #[allow(clippy::type_complexity)]
        pub on_geometry: RefCell<Option<Box<dyn Fn(PanelRect)>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Stage {
        const NAME: &'static str = "LumenStage";
        type Type = super::Stage;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("stage");
        }
    }

    impl ObjectImpl for Stage {
        fn dispose(&self) {
            if let Some(p) = self.panel.take() {
                p.unparent();
            }
        }
    }

    impl WidgetImpl for Stage {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            match &*self.panel.borrow() {
                Some(p) => {
                    let for_size = match orientation {
                        gtk::Orientation::Vertical if for_size > 0 => PANEL_WIDTH.min(for_size),
                        gtk::Orientation::Vertical => PANEL_WIDTH,
                        _ => -1,
                    };
                    let (min, _, _, _) = p.measure(orientation, for_size);
                    (min, min, -1, -1)
                }
                None => (0, 0, -1, -1),
            }
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let Some(panel) = self.panel.borrow().clone() else { return };
            let pw = PANEL_WIDTH.min((width - 32).max(200));
            let (_, nat, _, _) = panel.measure(gtk::Orientation::Vertical, pw);
            // Spotlight's position: about a fifth of the way down.
            let top = 48.max((height as f64 * 0.2) as i32).min((height - nat).max(0));
            let x = (width - pw) / 2;
            panel.size_allocate(&gtk::Allocation::new(x, top, pw, nat), -1);
            let rect = PanelRect { x, y: top, width: pw, height: nat };
            if rect != self.rect.get() {
                self.rect.set(rect);
                if let Some(f) = &*self.on_geometry.borrow() {
                    f(rect);
                }
            }
        }
    }
}

glib::wrapper! {
    /// Fills the (transparent, full-screen) window and places the panel like Spotlight does.
    pub struct Stage(ObjectSubclass<stage_imp::Stage>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Stage {
    pub fn new(panel: &impl IsA<gtk::Widget>) -> Self {
        let s: Stage = glib::Object::new();
        panel.set_parent(&s);
        s.imp().panel.replace(Some(panel.clone().upcast()));
        s.set_hexpand(true);
        s.set_vexpand(true);
        s
    }

    pub fn panel_rect(&self) -> PanelRect {
        self.imp().rect.get()
    }

    pub fn connect_geometry(&self, f: impl Fn(PanelRect) + 'static) {
        self.imp().on_geometry.replace(Some(Box::new(f)));
    }
}

// ── AutoHeight ─────────────────────────────────────────────────────────────

const HEIGHT_MS: f64 = 260.0;

mod auto_imp {
    use super::*;

    #[derive(Default)]
    pub struct AutoHeight {
        pub child: RefCell<Option<gtk::Widget>>,
        pub current: Cell<f64>,
        pub from: Cell<f64>,
        pub target: Cell<i32>,
        pub started: Cell<Option<Instant>>,
        pub ticking: Cell<bool>,
        pub primed: Cell<bool>,
        pub enabled: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AutoHeight {
        const NAME: &'static str = "LumenAutoHeight";
        type Type = super::AutoHeight;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("autoheight");
        }
    }

    impl ObjectImpl for AutoHeight {
        fn dispose(&self) {
            if let Some(c) = self.child.take() {
                c.unparent();
            }
        }
    }

    impl WidgetImpl for AutoHeight {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let Some(child) = self.child.borrow().clone() else { return (0, 0, -1, -1) };
            if orientation == gtk::Orientation::Horizontal {
                let (min, nat, _, _) = child.measure(orientation, -1);
                return (min, nat, -1, -1);
            }
            let (_, nat, _, _) = child.measure(orientation, for_size);
            if nat != self.target.get() || !self.primed.get() {
                let jump = !self.primed.get() || !self.enabled.get() || !animations_enabled();
                self.target.set(nat);
                self.primed.set(true);
                if jump {
                    self.current.set(nat as f64);
                    self.started.set(None);
                } else {
                    self.from.set(self.current.get());
                    self.started.set(Some(Instant::now()));
                    self.obj().start_ticking();
                }
            }
            let h = self.current.get().round() as i32;
            (h, h, -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            if let Some(child) = &*self.child.borrow() {
                let (_, nat, _, _) = child.measure(gtk::Orientation::Vertical, width);
                child.size_allocate(&gtk::Allocation::new(0, 0, width, nat.max(height)), -1);
            }
        }
    }
}

glib::wrapper! {
    /// Shows its child at the child's natural height, animating between heights.
    pub struct AutoHeight(ObjectSubclass<auto_imp::AutoHeight>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Default for AutoHeight {
    fn default() -> Self {
        Self::new()
    }
}

impl AutoHeight {
    pub fn new() -> Self {
        let a: AutoHeight = glib::Object::new();
        a.set_overflow(gtk::Overflow::Hidden);
        a.imp().enabled.set(true);
        a
    }

    pub fn set_child(&self, child: Option<&impl IsA<gtk::Widget>>) {
        if let Some(old) = self.imp().child.take() {
            old.unparent();
        }
        if let Some(c) = child {
            c.set_parent(self);
            self.imp().child.replace(Some(c.clone().upcast()));
        }
        self.queue_resize();
    }

    /// Turn animation off (while hidden), so the next show starts at the right size.
    pub fn set_animate(&self, on: bool) {
        self.imp().enabled.set(on);
    }

    fn start_ticking(&self) {
        let imp = self.imp();
        if imp.ticking.get() {
            return;
        }
        imp.ticking.set(true);
        self.add_tick_callback(|w, _clock| {
            let imp = w.imp();
            let Some(start) = imp.started.get() else {
                imp.ticking.set(false);
                return glib::ControlFlow::Break;
            };
            let t = start.elapsed().as_secs_f64() * 1000.0 / HEIGHT_MS;
            let target = imp.target.get() as f64;
            let from = imp.from.get();
            imp.current.set(from + (target - from) * ease_out(t));
            w.queue_resize();
            if t >= 1.0 {
                imp.current.set(target);
                imp.started.set(None);
                imp.ticking.set(false);
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }
}

// ── ListCanvas ─────────────────────────────────────────────────────────────

const SLIDE_MS: f64 = 170.0;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn lerp(a: Rect, b: Rect, t: f64) -> Rect {
        Rect { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t, w: a.w + (b.w - a.w) * t, h: a.h + (b.h - a.h) * t }
    }
}

mod canvas_imp {
    use super::*;

    #[derive(Default)]
    pub struct ListCanvas {
        pub content: RefCell<Option<gtk::Widget>>,
        pub highlight: RefCell<Option<gtk::Widget>>,
        pub light: RefCell<Option<gtk::Widget>>,
        pub selected: RefCell<Option<gtk::Widget>>,
        pub current: Cell<Option<Rect>>,
        pub from: Cell<Rect>,
        pub target: Cell<Option<Rect>>,
        pub started: Cell<Option<Instant>>,
        pub ticking: Cell<bool>,
        #[allow(clippy::type_complexity)]
        pub on_target: RefCell<Option<Box<dyn Fn(f64, f64)>>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ListCanvas {
        const NAME: &'static str = "LumenListCanvas";
        type Type = super::ListCanvas;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("listcanvas");
        }
    }

    impl ObjectImpl for ListCanvas {
        fn dispose(&self) {
            for slot in [&self.highlight, &self.light, &self.content] {
                if let Some(w) = slot.take() {
                    w.unparent();
                }
            }
        }
    }

    impl WidgetImpl for ListCanvas {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            match &*self.content.borrow() {
                Some(c) => {
                    let (min, nat, _, _) = c.measure(orientation, for_size);
                    (min, nat, -1, -1)
                }
                None => (0, 0, -1, -1),
            }
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let obj = self.obj();
            if let Some(c) = &*self.content.borrow() {
                c.size_allocate(&gtk::Allocation::new(0, 0, width, height), -1);
            }
            // Where the selected row is now.
            let target = self.selected.borrow().as_ref().and_then(|row| row.compute_bounds(&*obj)).map(|b| Rect {
                x: b.x() as f64,
                y: b.y() as f64,
                w: b.width() as f64,
                h: b.height() as f64,
            });
            if target != self.target.get() {
                self.target.set(target);
                if let Some(t) = target {
                    if let Some(f) = &*self.on_target.borrow() {
                        f(t.y, t.h);
                    }
                    match self.current.get() {
                        Some(cur) if animations_enabled() && obj.is_mapped() => {
                            self.from.set(cur);
                            self.started.set(Some(Instant::now()));
                            obj.start_ticking();
                        }
                        _ => {
                            self.current.set(Some(t));
                            self.started.set(None);
                        }
                    }
                } else {
                    self.current.set(None);
                }
            }
            let rect = self.current.get();
            let (Some(hl), Some(light)) = (self.highlight.borrow().clone(), self.light.borrow().clone()) else { return };
            hl.set_child_visible(rect.is_some());
            light.set_child_visible(rect.is_some());
            if let Some(r) = rect {
                let _ = hl.measure(gtk::Orientation::Horizontal, -1);
                let _ = light.measure(gtk::Orientation::Horizontal, -1);
                let t = gsk::Transform::new().translate(&graphene::Point::new(r.x as f32, r.y as f32));
                hl.allocate(r.w.round() as i32, r.h.round() as i32, -1, Some(t));
                let lh = (r.h * 0.46).round();
                let lt = gsk::Transform::new().translate(&graphene::Point::new(r.x as f32, (r.y + (r.h - lh) / 2.0) as f32));
                light.allocate(3, lh as i32, -1, Some(lt));
            }
        }
    }
}

glib::wrapper! {
    /// Draws a lit pane under the selected row and slides it when the selection moves.
    pub struct ListCanvas(ObjectSubclass<canvas_imp::ListCanvas>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ListCanvas {
    pub fn new(content: &impl IsA<gtk::Widget>) -> Self {
        let c: ListCanvas = glib::Object::new();
        let hl = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        hl.add_css_class("selection");
        hl.set_can_target(false);
        let light = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        light.add_css_class("light");
        light.set_can_target(false);
        // Children draw in order: the lit pane first, then the rows, then the light on top.
        hl.set_parent(&c);
        content.set_parent(&c);
        light.set_parent(&c);
        c.imp().highlight.replace(Some(hl.upcast()));
        c.imp().content.replace(Some(content.clone().upcast()));
        c.imp().light.replace(Some(light.upcast()));
        c
    }

    pub fn set_selected(&self, row: Option<&impl IsA<gtk::Widget>>) {
        self.imp().selected.replace(row.map(|r| r.clone().upcast()));
        self.queue_allocate();
    }

    /// Called with the selected row's top and height whenever it moves (for scrolling).
    pub fn connect_target(&self, f: impl Fn(f64, f64) + 'static) {
        self.imp().on_target.replace(Some(Box::new(f)));
    }

    fn start_ticking(&self) {
        let imp = self.imp();
        if imp.ticking.get() {
            return;
        }
        imp.ticking.set(true);
        self.add_tick_callback(|w, _| {
            let imp = w.imp();
            let (Some(start), Some(target)) = (imp.started.get(), imp.target.get()) else {
                imp.ticking.set(false);
                return glib::ControlFlow::Break;
            };
            let t = start.elapsed().as_secs_f64() * 1000.0 / SLIDE_MS;
            imp.current.set(Some(Rect::lerp(imp.from.get(), target, ease_out(t))));
            w.queue_allocate();
            if t >= 1.0 {
                imp.current.set(Some(target));
                imp.started.set(None);
                imp.ticking.set(false);
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }
}
