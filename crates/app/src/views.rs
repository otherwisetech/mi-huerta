//! Every tab of the window, the sidebar, the footer, the toast and About.
use crate::app::{HuertaApp, Tab};
use crate::theme::Tokens;
use crate::ui::{self, chip, filled_button, icon, labelled, outline_button, panel, section_title, segment, segmented};
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::IconName;
use gpui_component::button::Button;
use gpui_component::input::Input;
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use huerta::catalog::{Climate, HarvestFrom, Kind, Place, Rule, Season, Species};
use huerta::model::{Action, Garden, SupplyKind, TaskKind};
use huerta::schedule::{self, Bucket, Origin, RainNote, Task};

pub fn place_label(p: Place) -> String {
    match p {
        Place::Matera => i18n::t!("app.place.matera"),
        Place::Huerta => i18n::t!("app.place.huerta"),
        Place::Jardin => i18n::t!("app.place.jardin"),
        Place::Prado => i18n::t!("app.place.prado"),
    }
}

pub fn climate_label(c: Climate) -> String {
    match c {
        Climate::Calido => i18n::t!("app.climate.calido"),
        Climate::Templado => i18n::t!("app.climate.templado"),
        Climate::Frio => i18n::t!("app.climate.frio"),
    }
}

pub fn season_label(s: Season) -> String {
    match s {
        Season::Seca => i18n::t!("app.season.seca"),
        Season::Lluviosa => i18n::t!("app.season.lluviosa"),
    }
}

pub fn supply_kind_label(k: SupplyKind) -> String {
    match k {
        SupplyKind::Abono => i18n::t!("app.supply_kind.abono"),
        SupplyKind::Tierra => i18n::t!("app.supply_kind.tierra"),
        SupplyKind::Semillas => i18n::t!("app.supply_kind.semillas"),
        SupplyKind::Insecticida => i18n::t!("app.supply_kind.insecticida"),
        SupplyKind::Herramienta => i18n::t!("app.supply_kind.herramienta"),
        SupplyKind::Otro => i18n::t!("app.supply_kind.otro"),
    }
}

fn kind_label(k: Kind) -> String {
    match k {
        Kind::Hortaliza => i18n::t!("app.kind.hortaliza"),
        Kind::Aromatica => i18n::t!("app.kind.aromatica"),
        Kind::Medicinal => i18n::t!("app.kind.medicinal"),
        Kind::Frutal => i18n::t!("app.kind.frutal"),
        Kind::Ornamental => i18n::t!("app.kind.ornamental"),
        Kind::Prado => i18n::t!("app.kind.prado"),
    }
}

fn task_icon(k: TaskKind) -> IconName {
    match k {
        TaskKind::Water => IconName::Inbox,
        TaskKind::Fertilize => IconName::Plus,
        TaskKind::Prune => IconName::Minus,
        TaskKind::Mow => IconName::Menu,
        TaskKind::Harvest => IconName::Star,
    }
}

fn num(v: f64) -> String {
    let s = if v.fract() == 0.0 { format!("{v:.0}") } else { format!("{v:.1}") };
    if i18n::locale() == "en" { s } else { s.replace('.', ",") }
}

/// A dropdown over `options`; picking one calls `pick`.
fn picker<T: Clone + PartialEq + 'static>(id: &'static str, label: String, options: Vec<(T, String)>, current: Option<T>, weak: WeakEntity<HuertaApp>, pick: fn(&mut HuertaApp, T, &mut Context<HuertaApp>)) -> impl IntoElement {
    Button::new(id).outline().label(label).dropdown_menu(move |mut menu, _, _| {
        menu = menu.scrollable(true).max_h(px(380.));
        for (v, l) in &options {
            let (w, v2) = (weak.clone(), v.clone());
            menu = menu.item(PopupMenuItem::new(l.clone()).checked(current.as_ref() == Some(v)).on_click(move |_, _, cx| {
                let v = v2.clone();
                w.update(cx, |this, cx| pick(this, v, cx)).ok();
            }));
        }
        menu
    })
}

fn yes_no(t: &Tokens, id: &'static str, on: bool, cx: &mut Context<HuertaApp>, set: fn(&mut HuertaApp, bool, &mut Context<HuertaApp>)) -> Div {
    segmented(t).child(segment(t, (id, 1usize), on).child(i18n::t!("app.yes")).on_click(cx.listener(move |this, _, _, cx| set(this, true, cx)))).child(segment(t, (id, 0usize), !on).child(i18n::t!("app.no")).on_click(cx.listener(move |this, _, _, cx| set(this, false, cx))))
}

fn heading(t: &Tokens, title: String, sub: Option<String>) -> Div {
    div().flex().flex_col().gap(px(2.)).child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).child(title)).children(sub.map(|s| div().text_size(px(13.)).text_color(t.ink_muted).child(s)))
}

fn page(id: &'static str) -> Stateful<Div> {
    div().id(id).size_full().overflow_y_scroll().p(px(24.)).flex().flex_col().gap(px(16.))
}

/// "cada 3 días en matera, cálido, seca"
fn rule_text(r: &Rule) -> String {
    let mut q = Vec::new();
    q.extend(r.place.map(place_label));
    q.extend(r.climate.map(climate_label));
    q.extend(r.season.map(season_label));
    let every = i18n::t!("app.care.every", days = i18n::tn!("common.days", r.days));
    if q.is_empty() { every } else { format!("{every} · {}", q.join(", ")) } // i18n-ok: separator
}

impl HuertaApp {
    fn source_line(&self, t: &Tokens, id: SharedString, source: &str, reference: &str) -> Stateful<Div> {
        let src = self.catalog().source(source);
        let text = match src {
            Some(s) => i18n::t!("app.care.source", publisher = s.publisher, title = s.title, reference = reference),
            None => i18n::t!("app.care.source_unknown", source = source),
        };
        let url = src.map(|s| s.url.clone()).unwrap_or_default();
        div().id(id).text_size(px(11.)).line_height(px(15.)).text_color(t.ink_muted).child(text).when(!url.is_empty(), |d| d.cursor_pointer().hover(|s| s.underline()).on_click(move |_, _, cx| cx.open_url(&url)))
    }

    pub fn render_sidebar(&self, t: &Tokens, cx: &mut Context<Self>) -> impl IntoElement {
        let today = huerta::today();
        let due = self.garden().map(|g| schedule::agenda(g, self.catalog(), today).tasks.iter().filter(|x| x.due <= today).count()).unwrap_or(0);
        let low = self.garden().map(|g| g.low_supplies().len()).unwrap_or(0);
        let item = |tab: Tab, label: String, ic: IconName, badge: usize, cx: &mut Context<Self>| {
            otherwise_ui::NavItem::new(("tab", tab as usize), label).icon(ic).badge((badge > 0).then(|| badge.to_string())).active(self.tab == tab).on_click(cx.listener(move |this, _, _, cx| this.set_tab(tab, cx)))
        };
        let mut climate = Vec::new();
        if let Some(g) = self.garden() {
            climate.extend(g.settings.climate.map(climate_label));
            climate.push(season_label(g.settings.season));
        }
        let nav = otherwise_ui::NavGroup::new(None::<SharedString>)
            .item(item(Tab::Today, i18n::t!("app.tab.today"), IconName::Calendar, due, cx))
            .item(item(Tab::Plants, i18n::t!("app.tab.plants"), IconName::Heart, 0, cx))
            .item(item(Tab::Journal, i18n::t!("app.tab.journal"), IconName::BookOpen, 0, cx))
            .item(item(Tab::Supplies, i18n::t!("app.tab.supplies"), IconName::Inbox, low, cx))
            .item(item(Tab::Lawn, i18n::t!("app.tab.lawn"), IconName::Menu, 0, cx))
            .item(item(Tab::Settings, i18n::t!("app.tab.settings"), IconName::Settings, 0, cx));
        otherwise_ui::Sidebar::new("sidebar")
            .brand(ui::logo_glyph(otherwise_ui::tokens::size::LOGO_MARK, gpui::white()), crate::APP_NAME)
            .group(nav)
            .footer(otherwise_ui::assistant_button(t, "assistant", i18n::t!("app.agent.open"), None).on_click(cx.listener(|this, _, window, cx| this.toggle_panel(window, cx))))
            .when(!climate.is_empty(), |d| d.footer(div().px(px(otherwise_ui::tokens::space::S2)).text_size(px(otherwise_ui::tokens::text::CAPTION)).text_color(t.ink_muted).child(climate.join(" · ")))) // i18n-ok: separator
            .footer(otherwise_ui::privacy_note(t, i18n::t!("app.privacy.local"), None))
    }

    pub fn render_footer(&self, _t: &Tokens, cx: &mut Context<Self>) -> impl IntoElement {
        let status = match &self.store {
            Some(s) => {
                let st = s.status();
                let mut v = vec![s.dir.display().to_string()];
                v.extend(st.cloud.map(|c| c.provider.name().to_string()));
                if !s.writable() {
                    v.push(i18n::t!("app.storage.read_only"));
                }
                v.join(" · ") // i18n-ok: separator
            }
            None => i18n::t!("app.storage.none"),
        };
        otherwise_ui::AppFooter::new(crate::APP_NAME, i18n::t!("app.about.footer_link"))
            .version(crate::VERSION)
            .status(div().truncate().child(status))
            .item(div().flex_none().child(i18n::t!("app.surco")))
            .on_about(cx.listener(|this, _, _, cx| {
                this.about = true;
                cx.notify();
            }))
    }

    pub fn render_toast(&self, t: &Tokens) -> Option<impl IntoElement> {
        let (text, _) = self.toast.as_ref()?;
        Some(div().absolute().bottom(px(40.)).left_0().right_0().flex().justify_center().child(otherwise_ui::toast(t, text.clone())))
    }

    pub fn render_about(&self, t: &Tokens, cx: &mut Context<Self>) -> impl IntoElement {
        otherwise_ui::AboutModal::new(crate::APP_NAME, crate::VERSION, i18n::t!("app.about.by", company = crate::COMPANY))
            .logo(self.logo.clone())
            .description(i18n::t!("app.about.description"))
            .place(i18n::t!("app.about.place"))
            .website(i18n::t!("app.about.visit"), crate::COMPANY_URL)
            .vcity(i18n::t!("app.vcity.pitch"), i18n::t!("app.vcity.menu"), crate::VCITY_URL)
            .source(i18n::t!("app.about.source"), crate::REPO_URL)
            .extra(div().text_size(px(otherwise_ui::tokens::text::SMALL)).text_color(t.ink_muted).child(i18n::t!("app.surco")))
            .license(i18n::t!("app.about.license", company = crate::COMPANY))
            .on_dismiss(cx.listener(|this, _: &otherwise_ui::Dismiss, _, cx| {
                this.about = false;
                cx.notify();
            }))
    }

    pub fn render_body(&mut self, t: &Tokens, cx: &mut Context<Self>) -> AnyElement {
        if self.store.is_none() {
            let err = self.open_error.clone().unwrap_or_default();
            return page("no-garden").child(heading(t, i18n::t!("app.storage.none"), None)).child(ui::banner(t, ui::Tone::Warn, err)).child(outline_button(t, "open-other").child(i18n::t!("app.file.open")).on_click(cx.listener(|this, _, window, cx| this.open_garden(window, cx)))).into_any_element();
        }
        match self.tab {
            Tab::Today => self.render_today(t, cx).into_any_element(),
            Tab::Plants => self.render_plants(t, cx).into_any_element(),
            Tab::Journal => self.render_journal(t, cx).into_any_element(),
            Tab::Supplies => self.render_supplies(t, cx).into_any_element(),
            Tab::Lawn => self.render_lawn(t, cx).into_any_element(),
            Tab::Settings => self.render_settings(t, cx).into_any_element(),
        }
    }

    fn storage_banners(&self, t: &Tokens, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut v = Vec::new();
        if let Some(text) = self.read_only_text() {
            let can_take = matches!(self.store.as_ref().and_then(|s| s.read_only.as_ref()), Some(huerta::store::ReadOnly::Elsewhere(_) | huerta::store::ReadOnly::TakenOver));
            v.push(div().flex().items_center().gap(px(10.)).child(ui::banner(t, ui::Tone::Warn, text).flex_1()).when(can_take, |d| d.child(outline_button(t, "take-over").child(i18n::t!("app.storage.use_here")).on_click(cx.listener(|this, _, _, cx| this.take_over(cx))))).into_any_element());
        }
        if let Some(s) = &self.store {
            let n = s.status().conflicts.len();
            if n > 0 {
                v.push(div().flex().items_center().gap(px(10.)).child(ui::banner(t, ui::Tone::Warn, i18n::tn!("app.storage.conflicts", n)).flex_1()).child(outline_button(t, "show-conflicts").child(crate::app::reveal_label()).on_click(cx.listener(|this, _, _, cx| this.reveal(cx)))).into_any_element());
            }
        }
        v
    }

    // ── Hoy ──────────────────────────────────────────────────────────────────────────────────

    fn task_row(&self, t: &Tokens, ix: usize, task: &Task, today: chrono::NaiveDate, cx: &mut Context<Self>) -> impl IntoElement {
        let when = match task.bucket(today) {
            Bucket::Overdue => i18n::tn!("app.today.late_by", (today - task.due).num_days()),
            Bucket::Today => i18n::t!("app.today.today"),
            _ => task.due.format("%d/%m").to_string(),
        };
        let every = task.interval.map(|n| i18n::t!("app.care.every", days = i18n::tn!("common.days", n)));
        let origin = match &task.origin {
            Origin::User => i18n::t!("app.care.yours"),
            Origin::Catalog { source, .. } => self.catalog().source(source).map(|s| i18n::t!("app.care.per", publisher = s.publisher)).unwrap_or_default(),
        };
        let rain = task.rain.as_ref().map(|r| match r {
            RainNote::Rained { date, mm: Some(mm) } => i18n::t!("app.today.rained_mm", date = date.format("%d/%m"), mm = num(*mm)),
            RainNote::Rained { date, mm: None } => i18n::t!("app.today.rained", date = date.format("%d/%m")),
            RainNote::Forecast { date, mm } => i18n::t!("app.today.forecast", date = date.format("%d/%m"), mm = num(*mm)),
        });
        let late = task.bucket(today) == Bucket::Overdue;
        let mut meta: Vec<String> = every.into_iter().collect();
        if !origin.is_empty() {
            meta.push(origin);
        }
        if let Some(u) = task.until {
            meta.push(i18n::t!("app.today.until", date = u.format("%d/%m")));
        }
        let (k1, k2, k3) = (task.kind, task.kind, task.kind);
        let (id1, id2, id3) = (task.target.clone(), task.target.clone(), task.target.clone());
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .px(px(14.))
            .py(px(10.))
            .rounded(px(10.))
            .border_1()
            .border_color(t.border)
            .bg(t.surface)
            .child(div().flex().items_center().justify_center().size(px(30.)).rounded(px(8.)).bg(if task.kind == TaskKind::Water { t.info_soft } else { t.accent_soft }).child(icon(task_icon(task.kind), 15., if task.kind == TaskKind::Water { t.info } else { t.accent })))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(div().flex().items_center().gap(px(8.)).child(div().text_size(px(14.)).font_weight(FontWeight::MEDIUM).child(format!("{} · {}", mi_huerta_cli::task_label(task.kind), task.name))).child(chip(when, if late { t.danger } else { t.ink_muted }, if late { t.danger_soft } else { t.raised }))) // i18n-ok: separator
                    .child(div().text_size(px(11.)).text_color(t.ink_muted).child(meta.join(" · "))) // i18n-ok: separator
                    .children(rain.map(|r| div().text_size(px(11.)).text_color(t.info).child(r))),
            )
            .child(filled_button(t, ("done", ix), t.accent).child(i18n::t!("app.today.done")).on_click(cx.listener(move |this, _, _, cx| this.mark(k1, id1.clone(), Action::Done, cx))))
            .child(outline_button(t, ("snooze", ix)).child(i18n::t!("app.today.snooze")).on_click(cx.listener(move |this, _, _, cx| this.mark(k2, id2.clone(), Action::Snoozed { until: huerta::today() + chrono::Duration::days(1) }, cx))))
            .child(outline_button(t, ("skip", ix)).child(i18n::t!("app.today.skip")).on_click(cx.listener(move |this, _, _, cx| this.mark(k3, id3.clone(), Action::Skipped, cx))))
    }

    fn render_today(&self, t: &Tokens, cx: &mut Context<Self>) -> impl IntoElement {
        let today = huerta::today();
        let g = self.garden().cloned().unwrap_or_default();
        let a = schedule::agenda(&g, self.catalog(), today);
        let mut p = page("today").child(heading(t, i18n::t!("app.today.title"), Some(today.format("%d/%m/%Y").to_string()))).children(self.storage_banners(t, cx));
        p = p.child(self.render_rain_row(t, &g, cx));
        if g.plants.is_empty() && g.lawns.is_empty() {
            return p.child(
                panel(t)
                    .items_start()
                    .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(i18n::t!("app.today.empty_title")))
                    .child(div().text_size(px(13.)).text_color(t.ink_muted).child(i18n::t!("app.today.empty_body")))
                    .child(filled_button(t, "first-plant", t.accent).child(i18n::t!("app.plants.add")).on_click(cx.listener(|this, _, _, cx| this.set_tab(Tab::Plants, cx)))),
            );
        }
        let mut ix = 0;
        for (b, title) in [(Bucket::Overdue, i18n::t!("app.today.overdue")), (Bucket::Today, i18n::t!("app.today.today")), (Bucket::Week, i18n::t!("app.today.week"))] {
            let tasks = a.in_bucket(b, today);
            if tasks.is_empty() && b != Bucket::Today {
                continue;
            }
            let mut sec = div().flex().flex_col().gap(px(8.)).child(section_title(t, format!("{} ({})", title.to_uppercase(), tasks.len()))); // i18n-ok: count
            if tasks.is_empty() {
                sec = sec.child(div().text_size(px(13.)).text_color(t.ink_muted).child(i18n::t!("app.today.nothing")));
            }
            for task in tasks {
                sec = sec.child(self.task_row(t, ix, task, today, cx));
                ix += 1;
            }
            p = p.child(sec);
        }
        if !a.missing.is_empty() {
            let mut sec = panel(t).child(section_title(t, i18n::t!("app.today.missing").to_uppercase())).child(div().text_size(px(12.)).text_color(t.ink_muted).child(i18n::t!("app.today.missing_body")));
            for (i, m) in a.missing.iter().take(12).enumerate() {
                let id = m.target.clone();
                let is_lawn = m.kind == TaskKind::Mow;
                sec = sec.child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(10.))
                        .child(div().flex().flex_col().flex_1().gap(px(1.)).child(div().text_size(px(13.)).child(format!("{} · {}", mi_huerta_cli::task_label(m.kind), m.name))).children(m.hint.clone().map(|h| div().text_size(px(11.)).line_height(px(15.)).text_color(t.ink_muted).child(h)))) // i18n-ok: separator
                        .child(outline_button(t, ("define", i)).child(i18n::t!("app.today.define")).on_click(cx.listener(move |this, _, _, cx| {
                            if is_lawn {
                                this.set_tab(Tab::Lawn, cx);
                            } else {
                                this.select_plant(id.clone(), cx);
                                this.set_tab(Tab::Plants, cx);
                            }
                        }))),
                );
            }
            p = p.child(sec);
        }
        p.child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(8.))
                .child(outline_button(t, "ask-week").child(icon(IconName::Bot, 14., t.ink)).child(i18n::t!("app.agent.suggest_week")).on_click(cx.listener(|this, _, window, cx| this.ask(i18n::t!("app.agent.suggest_week"), window, cx))))
                .child(outline_button(t, "ask-lemon").child(icon(IconName::Bot, 14., t.ink)).child(i18n::t!("app.agent.suggest_lemon")).on_click(cx.listener(|this, _, window, cx| this.ask(i18n::t!("app.agent.suggest_lemon"), window, cx)))),
        )
    }

    fn render_rain_row(&self, t: &Tokens, g: &Garden, cx: &mut Context<Self>) -> impl IntoElement {
        let today = huerta::today();
        let w = &g.settings.weather;
        let info = if w.enabled {
            match &g.forecast {
                Some(f) => {
                    let days: Vec<String> = f.days.iter().filter(|d| d.date >= today).take(3).map(|d| i18n::t!("app.today.day_rain", date = d.date.format("%d/%m"), mm = num(d.rain_mm))).collect();
                    i18n::t!("app.today.forecast_line", place = w.place, days = days.join(" · ")) // i18n-ok: separator
                }
                None if self.weather_busy => i18n::t!("app.weather.loading"),
                None => i18n::t!("app.weather.no_forecast"),
            }
        } else {
            i18n::t!("app.today.weather_off")
        };
        let last = g.rain.iter().rev().find(|r| r.source == huerta::model::RainSource::Manual).map(|r| i18n::t!("app.today.last_rain", date = r.date.format("%d/%m")));
        div()
            .flex()
            .items_center()
            .flex_wrap()
            .gap(px(10.))
            .px(px(14.))
            .py(px(10.))
            .rounded(px(10.))
            .bg(t.info_soft)
            .text_size(px(12.))
            .child(icon(IconName::Info, 14., t.info))
            .child(div().flex_1().min_w(px(200.)).child(info))
            .children(last.map(|l| div().text_color(t.ink_muted).child(l)))
            .child(div().w(px(90.)).child(Input::new(&self.forms.rain_mm)))
            .child(outline_button(t, "log-rain").child(i18n::t!("app.today.it_rained")).on_click(cx.listener(|this, _, window, cx| this.log_rain(window, cx))))
    }

    // ── Plantas ──────────────────────────────────────────────────────────────────────────────

    fn render_plants(&self, t: &Tokens, cx: &mut Context<Self>) -> impl IntoElement {
        let g = self.garden().cloned().unwrap_or_default();
        let weak = cx.entity().downgrade();
        let lang = i18n::locale();
        let mut species: Vec<(Option<String>, String)> = self.catalog().plants.iter().map(|s| (Some(s.id.clone()), format!("{} ({})", s.name(lang), s.scientific))).collect(); // i18n-ok: name (scientific)
        species.sort_by(|a, b| a.1.cmp(&b.1));
        species.insert(0, (None, i18n::t!("app.plants.other_species")));
        let current = self.new_species.clone();
        let label = species.iter().find(|(v, _)| *v == current).map(|(_, l)| l.clone()).unwrap_or_default();
        let place = self.new_place;
        let mut places = segmented(t);
        for (i, p) in Place::ALL.into_iter().enumerate() {
            places = places.child(segment(t, ("new-place", i), p == place).child(place_label(p)).on_click(cx.listener(move |this, _, _, cx| {
                this.new_place = p;
                cx.notify();
            })));
        }
        let form = panel(t)
            .child(section_title(t, i18n::t!("app.plants.add").to_uppercase()))
            .child(labelled(t, i18n::t!("app.plants.species"), picker("species", label, species, Some(current), weak, |this, v, cx| {
                this.new_species = v;
                cx.notify();
            })))
            .child(labelled(t, i18n::t!("app.plants.name"), Input::new(&self.forms.plant_name)))
            .child(labelled(t, i18n::t!("app.plants.place"), places))
            .child(labelled(t, i18n::t!("app.plants.planted"), Input::new(&self.forms.plant_planted)))
            .child(labelled(t, i18n::t!("app.plants.notes"), Input::new(&self.forms.plant_notes)))
            .child(filled_button(t, "add-plant", t.accent).child(i18n::t!("app.plants.add")).on_click(cx.listener(|this, _, window, cx| this.add_plant(window, cx))));
        let mut list = div().flex().flex_col().gap(px(6.));
        let mut plants: Vec<_> = g.plants.iter().collect();
        plants.sort_by_key(|p| (p.archived, p.name.to_lowercase()));
        if plants.is_empty() {
            list = list.child(div().text_size(px(13.)).text_color(t.ink_muted).child(i18n::t!("app.plants.none")));
        }
        for (i, p) in plants.iter().enumerate() {
            let on = self.selected.as_deref() == Some(p.id.as_str());
            let sp = self.catalog().get(&p.species).map(|s| s.name(lang).to_string()).unwrap_or_default();
            let id = p.id.clone();
            let hover = t.border_strong;
            list = list.child(
                div()
                    .id(("plant", i))
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(9.))
                    .border_1()
                    .border_color(if on { t.accent } else { t.border })
                    .bg(t.surface)
                    .cursor_pointer()
                    .hover(move |s| s.border_color(hover))
                    .when(p.archived, |d| d.opacity(0.55))
                    .child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(p.name.clone()))
                    .child(div().text_size(px(11.)).text_color(t.ink_muted).child([sp, place_label(p.place)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "))) // i18n-ok: separator
                    .on_click(cx.listener(move |this, _, _, cx| this.select_plant(id.clone(), cx))),
            );
        }
        let left = div().id("plants-left").flex().flex_col().gap(px(12.)).w(px(330.)).flex_none().h_full().overflow_y_scroll().p(px(20.)).child(heading(t, i18n::t!("app.tab.plants"), None)).child(list).child(form);
        let detail = match self.selected.as_deref().and_then(|id| g.plant(id)) {
            Some(p) => self.render_plant_detail(t, p, cx).into_any_element(),
            None => page("plant-none").child(div().text_size(px(13.)).text_color(t.ink_muted).child(i18n::t!("app.plants.pick"))).into_any_element(),
        };
        div().flex().size_full().child(left).child(div().flex_1().min_w_0().h_full().border_l_1().border_color(t.border).child(detail))
    }

    fn care_rules(&self, t: &Tokens, sp: &Species) -> Div {
        let mut v = div().flex().flex_col().gap(px(8.));
        let mut n = 0usize;
        let mut topic = |title: String, lines: Vec<(String, Option<(String, String)>)>, hint: Option<&String>, v: Div| -> Div {
            let mut d = div().flex().flex_col().gap(px(2.)).child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child(title));
            for (text, src) in lines {
                d = d.child(div().text_size(px(13.)).child(text));
                if let Some((s, r)) = src {
                    n += 1;
                    d = d.child(self.source_line(t, SharedString::from(format!("src-{n}")), &s, &r)); // i18n-ok: element id
                }
            }
            if let Some(h) = hint {
                d = d.child(div().text_size(px(12.)).line_height(px(17.)).text_color(t.ink_muted).child(h.clone()));
            }
            v.child(d)
        };
        let rules = |rs: &[Rule]| rs.iter().map(|r| (rule_text(r), Some((r.source.clone(), r.reference.clone())))).collect::<Vec<_>>();
        v = topic(mi_huerta_cli::task_label(TaskKind::Water), rules(&sp.water), sp.hints.water.as_ref(), v);
        v = topic(mi_huerta_cli::task_label(TaskKind::Fertilize), rules(&sp.fertilize), sp.hints.fertilize.as_ref(), v);
        let prune = sp.prune.iter().map(|p| (i18n::t!("app.care.every", days = i18n::tn!("common.days", p.days)), Some((p.source.clone(), p.reference.clone())))).collect();
        v = topic(mi_huerta_cli::task_label(TaskKind::Prune), prune, sp.hints.prune.as_ref(), v);
        let harvest = sp
            .harvest
            .iter()
            .map(|h| {
                let from = if h.from == HarvestFrom::Trasplante { i18n::t!("app.care.from_transplant") } else { i18n::t!("app.care.from_sowing") };
                let range = match h.to_days {
                    Some(to) => i18n::t!("app.care.harvest_range", from = h.from_days, to = to, start = from),
                    None => i18n::t!("app.care.harvest_from", from = h.from_days, start = from),
                };
                (range, Some((h.source.clone(), h.reference.clone())))
            })
            .collect();
        v = topic(mi_huerta_cli::task_label(TaskKind::Harvest), harvest, sp.hints.harvest.as_ref(), v);
        if sp.kind == Kind::Prado || sp.mow.is_some() {
            let mow = sp.mow.iter().map(|m| (mow_text(m), Some((m.source.clone(), m.reference.clone())))).collect();
            v = topic(mi_huerta_cli::task_label(TaskKind::Mow), mow, sp.hints.mow.as_ref(), v);
        }
        v
    }

    fn render_plant_detail(&self, t: &Tokens, p: &huerta::model::Plant, cx: &mut Context<Self>) -> impl IntoElement {
        let lang = i18n::locale();
        let sp = self.catalog().get(&p.species);
        let mut places = segmented(t);
        for (i, pl) in Place::ALL.into_iter().enumerate() {
            let id = p.id.clone();
            places = places.child(segment(t, ("place", i), pl == p.place).child(place_label(pl)).on_click(cx.listener(move |this, _, _, cx| this.set_plant(id.clone(), cx, move |p| p.place = pl))));
        }
        let (id_cov, covered) = (p.id.clone(), p.covered);
        let cov = segmented(t)
            .child(segment(t, ("covered", 1usize), covered).child(i18n::t!("app.plants.covered")).on_click({
                let id = id_cov.clone();
                cx.listener(move |this, _, _, cx| this.set_plant(id.clone(), cx, |p| p.covered = true))
            }))
            .child(segment(t, ("covered", 0usize), !covered).child(i18n::t!("app.plants.outdoors")).on_click(cx.listener(move |this, _, _, cx| this.set_plant(id_cov.clone(), cx, |p| p.covered = false))));
        let sub = match sp {
            Some(s) => format!("{} · {} · {}", s.name(lang), s.scientific, kind_label(s.kind)), // i18n-ok: separator
            None => i18n::t!("app.plants.custom"),
        };
        let mut info = vec![];
        if let Some(d) = p.planted {
            info.push(i18n::t!("app.plants.planted_on", date = d.format("%d/%m/%Y")));
        }
        if let Some(s) = sp.filter(|s| !s.climates.is_empty()) {
            info.push(i18n::t!("app.plants.climates", climates = s.climates.iter().map(|c| climate_label(*c)).collect::<Vec<_>>().join(", "))); // i18n-ok: list separator
        }
        let intervals = div()
            .flex()
            .flex_wrap()
            .gap(px(10.))
            .child(labelled(t, mi_huerta_cli::task_label(TaskKind::Water), div().w(px(90.)).child(Input::new(&self.forms.water_days))))
            .child(labelled(t, mi_huerta_cli::task_label(TaskKind::Fertilize), div().w(px(90.)).child(Input::new(&self.forms.fertilize_days))))
            .child(labelled(t, mi_huerta_cli::task_label(TaskKind::Prune), div().w(px(90.)).child(Input::new(&self.forms.prune_days))))
            .child(labelled(t, mi_huerta_cli::task_label(TaskKind::Harvest), div().w(px(90.)).child(Input::new(&self.forms.harvest_days))));
        let store = self.store.as_ref();
        let mut photos = div().flex().flex_wrap().gap(px(8.));
        for rel in p.photos.iter().take(24) {
            if let Some(s) = store {
                photos = photos.child(img(s.photo_path(rel)).size(px(96.)).rounded(px(8.)).object_fit(ObjectFit::Cover));
            }
        }
        let (id_arch, archived) = (p.id.clone(), p.archived);
        let question = i18n::t!("app.agent.ask_photo", plant = p.name);
        page("plant-detail")
            .child(heading(t, p.name.clone(), Some(sub)))
            .children((!info.is_empty()).then(|| div().text_size(px(12.)).text_color(t.ink_muted).child(info.join(" · ")))) // i18n-ok: separator
            .child(div().flex().flex_wrap().gap(px(10.)).child(labelled(t, i18n::t!("app.plants.place"), places)).child(labelled(t, i18n::t!("app.plants.rain"), cov)))
            .child(panel(t).child(section_title(t, i18n::t!("app.care.title").to_uppercase())).child(match sp {
                Some(s) => self.care_rules(t, s),
                None => div().text_size(px(13.)).text_color(t.ink_muted).child(i18n::t!("app.care.custom")),
            }))
            .child(panel(t).child(section_title(t, i18n::t!("app.plants.yours").to_uppercase())).child(div().text_size(px(12.)).text_color(t.ink_muted).child(i18n::t!("app.plants.yours_body"))).child(intervals).child(labelled(t, i18n::t!("app.plants.notes"), Input::new(&self.forms.edit_notes))).child(filled_button(t, "save-plant", t.accent).child(i18n::t!("app.save")).on_click(cx.listener(|this, _, _, cx| this.save_plant_edits(cx)))))
            .child(panel(t).child(section_title(t, i18n::t!("app.photos.title").to_uppercase())).child(div().text_size(px(12.)).text_color(t.ink_muted).child(i18n::t!("app.photos.copied"))).child(photos).child(div().flex().gap(px(8.)).child(outline_button(t, "add-photos").child(i18n::t!("app.photos.add")).on_click(cx.listener(|this, _, window, cx| this.add_plant_photos(window, cx)))).child(outline_button(t, "ask-plant").child(icon(IconName::Bot, 14., t.ink)).child(question.clone()).on_click(cx.listener(move |this, _, window, cx| this.ask(question.clone(), window, cx))))))
            .child(outline_button(t, "archive").child(if archived { i18n::t!("app.plants.restore") } else { i18n::t!("app.plants.archive") }).on_click(cx.listener(move |this, _, _, cx| this.set_plant(id_arch.clone(), cx, move |p| p.archived = !archived))))
    }

    // ── Diario ───────────────────────────────────────────────────────────────────────────────

    fn render_journal(&self, t: &Tokens, cx: &mut Context<Self>) -> impl IntoElement {
        let g = self.garden().cloned().unwrap_or_default();
        let weak = cx.entity().downgrade();
        let mut targets: Vec<(Option<String>, String)> = vec![(None, i18n::t!("app.journal.whole_garden"))];
        targets.extend(g.plants.iter().filter(|p| !p.archived).map(|p| (Some(p.id.clone()), p.name.clone())));
        targets.extend(g.lawns.iter().map(|l| (Some(l.id.clone()), l.name.clone())));
        let current = self.journal_plant.clone();
        let label = targets.iter().find(|(v, _)| *v == current).map(|(_, l)| l.clone()).unwrap_or_default();
        let form = panel(t)
            .child(section_title(t, i18n::t!("app.journal.new").to_uppercase()))
            .child(labelled(t, i18n::t!("app.journal.about"), picker("journal-plant", label, targets, Some(current), weak, |this, v, cx| {
                this.journal_plant = v;
                cx.notify();
            })))
            .child(labelled(t, i18n::t!("app.journal.note"), Input::new(&self.forms.journal_note)))
            .child(labelled(t, i18n::t!("app.journal.pests"), Input::new(&self.forms.journal_pests)))
            .child(div().flex().gap(px(10.)).child(labelled(t, i18n::t!("app.journal.harvest"), div().w(px(110.)).child(Input::new(&self.forms.journal_amount)))).child(labelled(t, i18n::t!("app.journal.unit"), div().w(px(130.)).child(Input::new(&self.forms.journal_unit)))))
            .child(div().flex().items_center().gap(px(8.)).child(outline_button(t, "journal-photos").child(i18n::t!("app.photos.add")).on_click(cx.listener(|this, _, window, cx| this.add_journal_photos(window, cx)))).when(!self.journal_photos.is_empty(), |d| d.child(div().text_size(px(12.)).text_color(t.ink_muted).child(i18n::tn!("app.photos.ready", self.journal_photos.len())))))
            .child(filled_button(t, "add-journal", t.accent).child(i18n::t!("app.journal.add")).on_click(cx.listener(|this, _, window, cx| this.add_journal(window, cx))));
        let mut entries: Vec<_> = g.journal.iter().collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.date));
        let mut list = div().flex().flex_col().gap(px(8.));
        if entries.is_empty() {
            list = list.child(div().text_size(px(13.)).text_color(t.ink_muted).child(i18n::t!("app.journal.none")));
        }
        for j in entries.iter().take(200) {
            let who = j.plant.as_deref().and_then(|p| g.target_name(p)).map(str::to_string).unwrap_or_else(|| i18n::t!("app.journal.whole_garden"));
            let mut e = panel(t).gap(px(4.)).child(div().flex().items_center().gap(px(8.)).child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(who)).child(div().text_size(px(11.)).text_color(t.ink_muted).child(j.date.map(|d| d.format("%d/%m/%Y").to_string()).unwrap_or_default())));
            if !j.note.is_empty() {
                e = e.child(div().text_size(px(13.)).child(j.note.clone()));
            }
            if !j.pests.is_empty() {
                e = e.child(div().text_size(px(12.)).text_color(t.warn).child(i18n::t!("app.journal.pests_seen", pests = j.pests)));
            }
            if let Some(h) = &j.harvest {
                e = e.child(div().text_size(px(12.)).text_color(t.accent).child(i18n::t!("app.journal.harvested", amount = num(h.amount), unit = h.unit)));
            }
            if let Some(s) = &self.store
                && !j.photos.is_empty()
            {
                e = e.child(div().flex().gap(px(6.)).children(j.photos.iter().take(6).map(|r| img(s.photo_path(r)).size(px(72.)).rounded(px(6.)).object_fit(ObjectFit::Cover))));
            }
            list = list.child(e);
        }
        page("journal").child(heading(t, i18n::t!("app.tab.journal"), Some(i18n::t!("app.journal.sub")))).child(form).child(list)
    }

    // ── Insumos ──────────────────────────────────────────────────────────────────────────────

    fn render_supplies(&self, t: &Tokens, cx: &mut Context<Self>) -> impl IntoElement {
        let g = self.garden().cloned().unwrap_or_default();
        let weak = cx.entity().downgrade();
        let kinds: Vec<(SupplyKind, String)> = SupplyKind::ALL.into_iter().map(|k| (k, supply_kind_label(k))).collect();
        let form = panel(t)
            .child(section_title(t, i18n::t!("app.supplies.new").to_uppercase()))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .child(labelled(t, i18n::t!("app.supplies.kind"), picker("supply-kind", supply_kind_label(self.new_supply_kind), kinds, Some(self.new_supply_kind), weak.clone(), |this, k, cx| {
                        this.new_supply_kind = k;
                        cx.notify();
                    })))
                    .child(labelled(t, i18n::t!("app.supplies.name"), div().w(px(200.)).child(Input::new(&self.forms.supply_name))))
                    .child(labelled(t, i18n::t!("app.supplies.qty"), div().w(px(90.)).child(Input::new(&self.forms.supply_qty))))
                    .child(labelled(t, i18n::t!("app.supplies.unit"), div().w(px(90.)).child(Input::new(&self.forms.supply_unit))))
                    .child(labelled(t, i18n::t!("app.supplies.low"), div().w(px(90.)).child(Input::new(&self.forms.supply_low)))),
            )
            .child(filled_button(t, "add-supply", t.accent).child(i18n::t!("app.supplies.add")).on_click(cx.listener(|this, _, window, cx| this.add_supply(window, cx))));
        let mut list = div().flex().flex_col().gap(px(6.));
        if g.supplies.is_empty() {
            list = list.child(div().text_size(px(13.)).text_color(t.ink_muted).child(i18n::t!("app.supplies.none")));
        }
        for (i, s) in g.supplies.iter().enumerate() {
            let (a, b, c) = (s.id.clone(), s.id.clone(), s.id.clone());
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(14.))
                    .py(px(8.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(if s.is_low() { t.warn } else { t.border })
                    .bg(t.surface)
                    .child(div().flex().flex_col().flex_1().child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(s.name.clone())).child(div().text_size(px(11.)).text_color(t.ink_muted).child(supply_kind_label(s.kind))))
                    .when(s.is_low(), |d| d.child(chip(i18n::t!("app.supplies.running_low"), t.warn, t.warn_soft)))
                    .child(div().text_size(px(13.)).child(format!("{} {}", num(s.quantity), s.unit))) // i18n-ok: quantity unit
                    .child(outline_button(t, ("minus", i)).child("−").on_click(cx.listener(move |this, _, _, cx| this.adjust_supply(a.clone(), -1.0, cx)))) // i18n-ok: symbol
                    .child(outline_button(t, ("plus", i)).child("+").on_click(cx.listener(move |this, _, _, cx| this.adjust_supply(b.clone(), 1.0, cx)))) // i18n-ok: symbol
                    .child(outline_button(t, ("buy", i)).child(i18n::t!("app.supplies.buy")).on_click(cx.listener(move |this, _, _, cx| {
                        this.buy_supply = Some(c.clone());
                        cx.notify();
                    }))),
            );
        }
        let buying = self.buy_supply.as_deref().and_then(|id| g.supply(id)).map(|s| s.name.clone());
        let purchase = buying.map(|name| {
            panel(t)
                .child(section_title(t, i18n::t!("app.supplies.purchase_of", name = name).to_uppercase()))
                .child(div().flex().flex_wrap().gap(px(10.)).child(labelled(t, i18n::t!("app.supplies.qty"), div().w(px(90.)).child(Input::new(&self.forms.buy_qty)))).child(labelled(t, i18n::t!("app.supplies.cost"), div().w(px(130.)).child(Input::new(&self.forms.buy_cost)))).child(labelled(t, i18n::t!("app.supplies.shop"), div().w(px(200.)).child(Input::new(&self.forms.buy_shop)))))
                .child(filled_button(t, "add-purchase", t.accent).child(i18n::t!("app.supplies.record")).on_click(cx.listener(|this, _, window, cx| this.add_purchase(window, cx))))
        });
        let mut history = div().flex().flex_col().gap(px(4.));
        let mut ps: Vec<_> = g.purchases.iter().collect();
        ps.sort_by_key(|e| std::cmp::Reverse(e.date));
        for p in ps.iter().take(50) {
            let name = g.supply(&p.supply).map(|s| s.name.clone()).unwrap_or_default();
            history = history.child(div().flex().gap(px(10.)).text_size(px(12.)).child(div().w(px(80.)).text_color(t.ink_muted).child(p.date.map(|d| d.format("%d/%m/%Y").to_string()).unwrap_or_default())).child(div().flex_1().child(format!("{name} · {}", num(p.quantity)))).child(div().text_color(t.ink_muted).child(p.shop.clone())).child(div().child(format!("$ {} {}", num(p.cost), p.currency)))); // i18n-ok: money
        }
        let spent = g.spent().iter().map(|(c, v)| format!("$ {} {c}", num(*v))).collect::<Vec<_>>().join(" · "); // i18n-ok: money
        page("supplies")
            .child(heading(t, i18n::t!("app.tab.supplies"), Some(i18n::t!("app.supplies.sub"))))
            .child(list)
            .children(purchase)
            .child(form)
            .when(!g.purchases.is_empty(), |d| d.child(panel(t).child(section_title(t, i18n::t!("app.supplies.history").to_uppercase())).child(history).child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child(i18n::t!("app.supplies.spent", total = spent)))))
    }

    // ── Prado ────────────────────────────────────────────────────────────────────────────────

    fn render_lawn(&self, t: &Tokens, cx: &mut Context<Self>) -> impl IntoElement {
        let g = self.garden().cloned().unwrap_or_default();
        let lawn_sp = self.catalog().plants.iter().find(|s| s.kind == Kind::Prado);
        let advice = lawn_sp.map(|s| {
            let mut p = panel(t).child(section_title(t, i18n::t!("app.lawn.advice", species = s.name(i18n::locale())).to_uppercase()));
            if let Some(m) = &s.mow {
                p = p.child(div().text_size(px(13.)).child(mow_text(m))).child(self.source_line(t, "lawn-src".into(), &m.source, &m.reference));
            }
            p.children(s.hints.mow.clone().map(|h| div().text_size(px(12.)).text_color(t.ink_muted).child(h)))
        });
        let form = panel(t)
            .child(section_title(t, i18n::t!("app.lawn.new").to_uppercase()))
            .child(div().flex().flex_wrap().gap(px(10.)).child(labelled(t, i18n::t!("app.lawn.name"), div().w(px(200.)).child(Input::new(&self.forms.lawn_name)))).child(labelled(t, i18n::t!("app.lawn.every"), div().w(px(110.)).child(Input::new(&self.forms.lawn_days)))).child(labelled(t, i18n::t!("app.lawn.height"), div().w(px(110.)).child(Input::new(&self.forms.lawn_height)))))
            .child(filled_button(t, "add-lawn", t.accent).child(i18n::t!("app.lawn.add")).on_click(cx.listener(|this, _, window, cx| this.add_lawn(window, cx))));
        let mut list = div().flex().flex_col().gap(px(6.));
        for (i, l) in g.lawns.iter().enumerate() {
            let id = l.id.clone();
            let mut meta = vec![];
            meta.extend(l.mow_days.map(|d| i18n::t!("app.care.every", days = i18n::tn!("common.days", d))));
            meta.extend(l.height_cm.map(|h| i18n::t!("app.lawn.at_height", cm = num(h))));
            meta.push(match l.last_mowed {
                Some(d) => i18n::t!("app.lawn.last", date = d.format("%d/%m/%Y")),
                None => i18n::t!("app.lawn.never"),
            });
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(14.))
                    .py(px(10.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(t.border)
                    .bg(t.surface)
                    .child(div().flex().flex_col().flex_1().child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(l.name.clone())).child(div().text_size(px(11.)).text_color(t.ink_muted).child(meta.join(" · ")))) // i18n-ok: separator
                    .child(filled_button(t, ("mowed", i), t.accent).child(i18n::t!("app.lawn.mowed_today")).on_click(cx.listener(move |this, _, _, cx| this.mark(TaskKind::Mow, id.clone(), Action::Done, cx)))),
            );
        }
        page("lawn").child(heading(t, i18n::t!("app.tab.lawn"), Some(i18n::t!("app.lawn.sub")))).children(advice).child(list).child(form)
    }

    // ── Ajustes ──────────────────────────────────────────────────────────────────────────────

    fn render_settings(&self, t: &Tokens, cx: &mut Context<Self>) -> impl IntoElement {
        let g = self.garden().cloned().unwrap_or_default();
        let mut climate = segmented(t).child(segment(t, ("climate", 9usize), g.settings.climate.is_none()).child(i18n::t!("app.climate.unset")).on_click(cx.listener(|this, _, _, cx| this.set_climate(None, cx))));
        for (i, c) in Climate::ALL.into_iter().enumerate() {
            climate = climate.child(segment(t, ("climate", i), g.settings.climate == Some(c)).child(climate_label(c)).on_click(cx.listener(move |this, _, _, cx| this.set_climate(Some(c), cx))));
        }
        let floors = self.catalog().climates.as_ref().map(|v| {
            let alt = |k: &str| v.get(k).and_then(|f| Some((f.get("min_m")?.as_integer()?, f.get("max_m")?.as_integer()?)));
            let parts: Vec<String> = Climate::ALL.iter().filter_map(|c| alt(c.id()).map(|(a, b)| i18n::t!("app.climate.range", name = climate_label(*c), from = a, to = b))).collect();
            let src = v.get("source").and_then(|s| s.as_str()).unwrap_or_default().to_string();
            let reference = v.get("ref").and_then(|s| s.as_str()).unwrap_or_default().to_string();
            (parts.join(" · "), src, reference) // i18n-ok: separator
        });
        let mut season = segmented(t);
        for (i, s) in [Season::Seca, Season::Lluviosa].into_iter().enumerate() {
            season = season.child(segment(t, ("season", i), g.settings.season == s).child(season_label(s)).on_click(cx.listener(move |this, _, _, cx| this.set_season(s, cx))));
        }
        let w = &g.settings.weather;
        let url = match (w.lat, w.lon) {
            (Some(a), Some(b)) => huerta::weather::forecast_url(a, b),
            _ => i18n::t!("app.settings.no_place"),
        };
        let mut found = div().flex().flex_col().gap(px(4.));
        for (i, f) in self.found.iter().enumerate() {
            let f2 = f.clone();
            found = found.child(outline_button(t, ("found", i)).child(format!("{} — {} ({:.1}, {:.1})", f.name, f.region, f.lat, f.lon)).on_click(cx.listener(move |this, _, _, cx| this.choose_city(f2.clone(), cx)))); // i18n-ok: place line
        }
        let weather = panel(t)
            .child(section_title(t, i18n::t!("app.settings.weather").to_uppercase()))
            .child(div().text_size(px(13.)).line_height(px(19.)).child(i18n::t!("app.settings.weather_body")))
            .child(yes_no(t, "weather", w.enabled, cx, |this, on, cx| this.set_weather(on, cx)))
            .child(ui::banner(t, ui::Tone::Neutral, i18n::t!("app.settings.weather_privacy")))
            .child(labelled(t, i18n::t!("app.settings.what_is_sent"), div().text_size(px(11.)).font_family("Menlo").text_color(t.ink_muted).child(url))) // i18n-ok: font name
            .when(w.enabled, |d| {
                d.child(div().text_size(px(12.)).child(if w.place.is_empty() { i18n::t!("app.settings.no_place") } else { i18n::t!("app.settings.place_is", place = w.place) }))
                    .child(div().flex().gap(px(8.)).items_end().child(labelled(t, i18n::t!("app.settings.city"), div().w(px(240.)).child(Input::new(&self.forms.city)))).child(outline_button(t, "search-city").child(if self.weather_busy { i18n::t!("app.weather.loading") } else { i18n::t!("app.settings.search") }).on_click(cx.listener(|this, _, _, cx| this.search_city(cx)))).child(outline_button(t, "refresh-weather").child(i18n::t!("app.settings.refresh")).on_click(cx.listener(|this, _, _, cx| this.refresh_weather(cx)))))
                    .child(found)
            })
            .child(div().flex().gap(px(8.)).items_end().child(labelled(t, i18n::t!("app.settings.skip_mm", mm = num(w.skip_mm)), div().w(px(110.)).child(Input::new(&self.forms.skip_mm)))).child(outline_button(t, "save-skip").child(i18n::t!("app.save")).on_click(cx.listener(|this, _, _, cx| this.save_skip_mm(cx)))));
        let mut langs = segmented(t);
        for (i, (code, name)) in i18n::available().into_iter().enumerate() {
            langs = langs.child(segment(t, ("lang", i), i18n::locale() == code).child(name).on_click(cx.listener(move |this, _, _, cx| this.set_language(code, cx))));
        }
        let storage = self.store.as_ref().map(|s| {
            let st = s.status();
            let cloud = st.cloud.map(|c| i18n::t!("app.storage.cloud", provider = c.provider.name())).unwrap_or_else(|| i18n::t!("app.storage.local"));
            panel(t)
                .child(section_title(t, i18n::t!("app.storage.title").to_uppercase()))
                .child(div().text_size(px(13.)).child(s.dir.display().to_string()))
                .child(div().text_size(px(12.)).text_color(t.ink_muted).child(cloud))
                .child(div().text_size(px(12.)).text_color(t.ink_muted).child(i18n::t!("app.storage.body")))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(8.))
                        .child(outline_button(t, "save-as").child(i18n::t!("app.file.save_as")).on_click(cx.listener(|this, _, window, cx| this.save_as(window, cx))))
                        .child(outline_button(t, "move").child(i18n::t!("app.file.move")).on_click(cx.listener(|this, _, window, cx| this.move_garden(window, cx))))
                        .child(outline_button(t, "open").child(i18n::t!("app.file.open")).on_click(cx.listener(|this, _, window, cx| this.open_garden(window, cx))))
                        .child(outline_button(t, "reveal").child(crate::app::reveal_label()).on_click(cx.listener(|this, _, _, cx| this.reveal(cx)))),
                )
        });
        page("settings")
            .child(heading(t, i18n::t!("app.tab.settings"), None))
            .children(self.storage_banners(t, cx))
            .child(
                panel(t)
                    .child(section_title(t, i18n::t!("app.settings.garden").to_uppercase()))
                    .child(labelled(t, i18n::t!("app.settings.climate"), climate))
                    .children(floors.map(|(text, src, r)| div().flex().flex_col().gap(px(2.)).child(div().text_size(px(12.)).text_color(t.ink_muted).child(text)).child(self.source_line(t, "climate-src".into(), &src, &r))))
                    .child(labelled(t, i18n::t!("app.settings.season"), season)),
            )
            .child(weather)
            .child(
                panel(t)
                    .child(section_title(t, i18n::t!("app.settings.otherwise").to_uppercase()))
                    .child(labelled(t, i18n::t!("app.settings.calendar"), yes_no(t, "calendar", g.settings.calendar, cx, |this, on, cx| {
                        this.change(cx, |g| {
                            g.settings.calendar = on;
                            Ok(())
                        });
                    })))
                    .child(labelled(t, i18n::t!("app.settings.notifications"), yes_no(t, "notify", g.settings.notifications, cx, |this, on, cx| {
                        this.change(cx, |g| {
                            g.settings.notifications = on;
                            Ok(())
                        });
                    })))
                    .child(div().text_size(px(12.)).text_color(t.ink_muted).child(i18n::t!("app.settings.helper_body"))),
            )
            .child(panel(t).child(labelled(t, i18n::t!("app.settings.language"), langs)))
            .children(storage)
            .child(div().flex().gap(px(14.)).text_size(px(12.)).child(ui::link(t, "settings-vcity").child(i18n::t!("app.vcity.menu")).on_click(|_, _, cx| cx.open_url(crate::VCITY_URL))).child(div().text_color(t.ink_muted).child(i18n::t!("app.surco"))))
    }
}

fn mow_text(m: &huerta::catalog::Mow) -> String {
    let mut v = vec![];
    if let Some(d) = m.days {
        v.push(i18n::t!("app.care.every", days = i18n::tn!("common.days", d)));
    }
    match (m.height_min_cm, m.height_max_cm) {
        (Some(a), Some(b)) => v.push(i18n::t!("app.care.height_range", from = num(a), to = num(b))),
        (Some(a), None) | (None, Some(a)) => v.push(i18n::t!("app.lawn.at_height", cm = num(a))),
        _ => {}
    }
    v.join(" · ") // i18n-ok: separator
}

