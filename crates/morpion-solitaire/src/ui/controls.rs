use crate::game::rules::{TouchMode, Variant};
use crate::i18n::{set_language, LANGUAGE_LOADER};
use crate::ui::icons::{self, Icon};
use egui::{Color32, RichText, Ui};
use i18n_embed_fl::fl;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SearchAlgo {
    Nrpa,
    Beam,
    Systematic,
    /// Perturbation (large-neighbourhood) search around the loaded game. Native
    /// only (it drives time-bounded inner NRPA searches via OS threads).
    Perturbation,
}

/// Where a search begins — replaces the old warm-start + reset-to-initial
/// checkboxes with a single coherent choice (the valid set depends on the algo).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StartPoint {
    /// A fresh empty cross.
    Empty,
    /// A fresh empty cross, with the NRPA policy seeded by the loaded game
    /// (the loaded game is a prior, not the start). NRPA only.
    Seeded,
    /// Continue from the currently loaded position.
    Continue,
}

/// The starting points that make sense for `algo`, in display order. Perturbation
/// always perturbs the loaded game, so it offers no choice (empty slice).
pub fn start_points_for(algo: SearchAlgo) -> &'static [StartPoint] {
    match algo {
        SearchAlgo::Nrpa => &[StartPoint::Empty, StartPoint::Seeded, StartPoint::Continue],
        SearchAlgo::Systematic | SearchAlgo::Beam => &[StartPoint::Empty, StartPoint::Continue],
        SearchAlgo::Perturbation => &[],
    }
}

/// What the "Resume" button will pick up, for a non-opaque label.
pub struct ResumeInfo {
    pub algo: SearchAlgo,
    pub age: Duration,
}

/// Output format for both the clipboard ("Copy") and the file export
/// ("Export…"). The same selector drives both actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExportFormat {
    /// Compact `MS1:` save string (the MSR record format).
    Msr,
    /// Human-readable JSON record.
    Json,
    /// Legacy Pentasol text format.
    Pentasol,
    /// Vector image (SVG document).
    Svg,
    /// Raster image (PNG). Native only — the web build has no rasteriser.
    Png,
}

/// The formats offered, in display order. PNG needs the native rasteriser, so it
/// is omitted on the web (where export is text-clipboard only).
pub fn export_formats() -> &'static [ExportFormat] {
    #[cfg(not(target_arch = "wasm32"))]
    {
        &[
            ExportFormat::Msr,
            ExportFormat::Json,
            ExportFormat::Pentasol,
            ExportFormat::Svg,
            ExportFormat::Png,
        ]
    }
    #[cfg(target_arch = "wasm32")]
    {
        &[
            ExportFormat::Msr,
            ExportFormat::Json,
            ExportFormat::Pentasol,
            ExportFormat::Svg,
        ]
    }
}

/// Short, untranslated label for a format (these are proper names / file types).
pub fn export_format_label(f: ExportFormat) -> &'static str {
    match f {
        ExportFormat::Msr => "MSR",
        ExportFormat::Json => "JSON",
        ExportFormat::Pentasol => "Pentasol",
        ExportFormat::Svg => "SVG",
        ExportFormat::Png => "PNG",
    }
}

pub struct ControlsInput {
    pub variant: Variant,
    pub algo: SearchAlgo,
    /// Whether a search result preview is on the board (read-only until loaded).
    pub showing_preview: bool,
    /// Where the next search will begin.
    pub start_point: StartPoint,
    /// Whether a non-empty game is loaded (enables Seeded/Continue and lets
    /// Perturbation run).
    pub warm_available: bool,
    /// Whether the loaded position is already terminal (no legal moves) — then
    /// "Continue" has nothing to explore.
    pub loaded_terminal: bool,
    /// Score-aligned labels of the known records, for the load dropdown.
    pub record_names: Vec<String>,
    /// Whether the record-beaten alarm is currently sounding (shows Silence).
    pub alarm_active: bool,
    /// Format used by both Copy (clipboard) and Export (file).
    pub export_format: ExportFormat,
    /// Current UI theme (drives the sun/moon toggle icon).
    pub dark_mode: bool,
    pub score: usize,
    pub legal_count: usize,
    pub search_running: bool,
    /// Whether the running search is currently paused (idling at a boundary).
    pub search_paused: bool,
    pub nodes_explored: u64,
    pub best_search_score: u32,
    pub nodes_per_sec: f64,
    pub elapsed: Duration,
    pub records: Vec<(u32, Duration)>,
    /// Whether search checkpoint/resume is available (native only).
    pub checkpoint_supported: bool,
    /// Present when a saved checkpoint exists on disk (enables & annotates Resume).
    pub resume: Option<ResumeInfo>,
}

#[derive(Default)]
pub struct ControlsOutput {
    pub new_game: Option<Variant>,
    pub set_algo: Option<SearchAlgo>,
    pub set_start_point: Option<StartPoint>,
    /// Index into `record_names` of a record the user chose to load.
    pub load_record: Option<usize>,
    pub start_search: bool,
    pub stop_search: bool,
    /// Toggle the cooperative pause on the running search.
    pub toggle_pause: bool,
    pub load_best: bool,
    /// Drop the result preview and reveal the editable played game again.
    pub dismiss_preview: bool,
    pub checkpoint: bool,
    pub resume_search: bool,
    pub set_export_format: Option<ExportFormat>,
    /// Copy the position to the clipboard in the selected format.
    pub copy: bool,
    /// Export the position to a file in the selected format (native only).
    pub export_file: bool,
    pub import: bool,
    pub silence_alarm: bool,
    pub toggle_theme: bool,
    pub show_shortcuts: bool,
    pub show_rules: bool,
}

pub fn show(ui: &mut Ui, input: &ControlsInput) -> ControlsOutput {
    let mut out = ControlsOutput::default();
    let l = &*LANGUAGE_LOADER;
    let sep = num_sep();

    ui.add_space(8.0);
    ui.heading(fl!(l, "app-title"));

    // Top row: language switcher (populated from the bundled locales), a theme
    // toggle, and a keyboard-shortcuts help button.
    ui.horizontal(|ui| {
        ui.label(fl!(l, "language-label"));
        let current = crate::i18n::current_language();
        egui::ComboBox::from_id_salt("language")
            .selected_text(crate::i18n::language_endonym(&current))
            .show_ui(ui, |ui| {
                for lang in crate::i18n::available_languages() {
                    let name = crate::i18n::language_endonym(&lang);
                    if ui.selectable_label(current == lang, name).clicked() {
                        set_language(&lang);
                    }
                }
            });
        // Sun while dark (→ switch to light), Moon while light (→ switch to dark).
        let theme_icon = if input.dark_mode {
            Icon::Sun
        } else {
            Icon::Moon
        };
        if icons::icon_button(ui, theme_icon, false, true)
            .on_hover_text(fl!(l, "btn-theme"))
            .clicked()
        {
            out.toggle_theme = true;
        }
        if ui
            .button("?")
            .on_hover_text(fl!(l, "btn-shortcuts"))
            .clicked()
        {
            out.show_shortcuts = true;
        }
        if icons::icon_button(ui, Icon::Info, false, true)
            .on_hover_text(fl!(l, "rules-title"))
            .clicked()
        {
            out.show_rules = true;
        }
    });

    // Record-beaten alarm: a prominent Silence button while it sounds.
    if input.alarm_active {
        let btn = egui::Button::new(
            RichText::new(fl!(l, "btn-silence"))
                .strong()
                .color(Color32::WHITE),
        )
        .fill(Color32::from_rgb(200, 40, 40));
        if ui.add_sized([ui.available_width(), 30.0], btn).clicked() {
            out.silence_alarm = true;
        }
    }

    ui.separator();
    ui.add_space(6.0);
    ui.heading(fl!(l, "game-section"));
    ui.add_space(4.0);

    // Score
    ui.label(
        RichText::new(format!("{} : {}", fl!(l, "score-label"), input.score))
            .size(20.0)
            .strong(),
    );
    // "Available moves" is only meaningful while editing by hand; a search result
    // (live or finished) on the board makes it noise, so hide it until we're back
    // in manual mode.
    if !input.showing_preview {
        ui.label(format!(
            "{} : {}",
            fl!(l, "legal-moves-label"),
            input.legal_count
        ));
    }
    ui.add_space(10.0);

    // Variant
    ui.label(RichText::new(fl!(l, "variant-label")).strong());
    ui.horizontal_wrapped(|ui| {
        for v in [Variant::T5, Variant::D5, Variant::T4, Variant::D4] {
            let mode = match v.touch_mode {
                TouchMode::Touching => fl!(l, "touch-touching"),
                TouchMode::Disjoint => fl!(l, "touch-disjoint"),
            };
            let tip = fl!(l, "variant-tip", len = (v.len() as i64), mode = mode);
            if ui
                .selectable_label(input.variant == v, v.name())
                .on_hover_text(tip)
                .clicked()
            {
                out.new_game = Some(v);
            }
        }
    });
    ui.add_space(10.0);

    // Document actions as icon buttons (New / Copy / Export / Import); undo/redo,
    // rotate/flip/recenter and the arrows/numbers toggles live overlaid on the
    // board. One format selector drives both Copy (clipboard) and Export (file).
    ui.horizontal(|ui| {
        ui.label(fl!(l, "format-label"));
        egui::ComboBox::from_id_salt("export_format")
            .selected_text(export_format_label(input.export_format))
            .show_ui(ui, |ui| {
                for &f in export_formats() {
                    if ui
                        .selectable_label(input.export_format == f, export_format_label(f))
                        .clicked()
                    {
                        out.set_export_format = Some(f);
                    }
                }
            });
    });
    ui.horizontal(|ui| {
        if icons::icon_button(ui, Icon::New, false, true)
            .on_hover_text(format!("{} ({}N)", fl!(l, "btn-new"), crate::ui::cmd_key()))
            .clicked()
        {
            out.new_game = Some(input.variant);
        }
        if icons::icon_button(ui, Icon::Copy, false, true)
            .on_hover_text(fl!(l, "btn-copy"))
            .clicked()
        {
            out.copy = true;
        }
        // Save to a file: a native save dialog, or a browser download on the web.
        if icons::icon_button(ui, Icon::Export, false, true)
            .on_hover_text(format!(
                "{} ({}S)",
                fl!(l, "btn-export-file"),
                crate::ui::cmd_key()
            ))
            .clicked()
        {
            out.export_file = true;
        }
        if icons::icon_button(ui, Icon::Import, false, true)
            .on_hover_text(fl!(l, "btn-import"))
            .clicked()
        {
            out.import = true;
        }
    });
    ui.add_space(4.0);

    // Load a known record game (compiled in). Disabled when the selected variant
    // has none; the count is shown so it's clear how many are on offer.
    let n_records = input.record_names.len();
    ui.add_enabled_ui(n_records > 0, |ui| {
        egui::ComboBox::from_id_salt("load_record")
            .selected_text(format!("{} ({n_records})", fl!(l, "load-record")))
            .show_ui(ui, |ui| {
                for (i, name) in input.record_names.iter().enumerate() {
                    if ui
                        .selectable_label(false, egui::RichText::new(name).monospace())
                        .clicked()
                    {
                        out.load_record = Some(i);
                    }
                }
            });
    });
    ui.add_space(10.0);

    // Solver controls — always available; running a search shows its result as
    // a read-only preview on the board.
    {
        ui.separator();
        ui.add_space(6.0);
        ui.heading(fl!(l, "search-section"));
        ui.add_space(4.0);
        ui.label(RichText::new(fl!(l, "algo-label")).strong());
        let algo_label = |a: SearchAlgo| match a {
            SearchAlgo::Nrpa => fl!(l, "algo-nrpa"),
            SearchAlgo::Beam => fl!(l, "algo-beam"),
            SearchAlgo::Systematic => fl!(l, "algo-systematic"),
            SearchAlgo::Perturbation => fl!(l, "algo-perturbation"),
        };
        ui.add_enabled_ui(!input.search_running, |ui| {
            // Ordered simplest/exact → most sophisticated heuristic, matching the
            // docs' Search-algorithms page.
            let mut algos = vec![SearchAlgo::Systematic, SearchAlgo::Beam, SearchAlgo::Nrpa];
            // Perturbation is native-only (it uses OS threads).
            if input.checkpoint_supported {
                algos.push(SearchAlgo::Perturbation);
            }
            egui::ComboBox::from_id_salt("algo")
                .selected_text(algo_label(input.algo))
                .show_ui(ui, |ui| {
                    for a in algos {
                        if ui
                            .selectable_label(input.algo == a, algo_label(a))
                            .clicked()
                        {
                            out.set_algo = Some(a);
                        }
                    }
                });
        });
        // Engine-tuning options, rendered generically from the plugin registry: a new
        // plugin option appears here with no edit to this file, and only the options in
        // scope for the chosen algorithm show (docs/plugin-framework.md).
        render_search_options(ui, input.algo, !input.search_running);

        // Starting point — the valid set depends on the algorithm. Perturbation
        // always perturbs the loaded game, so it shows a note instead of a choice.
        ui.add_space(6.0);
        ui.label(RichText::new(fl!(l, "start-point-label")).strong());
        let options = start_points_for(input.algo);
        if options.is_empty() {
            ui.label(RichText::new(fl!(l, "perturbation-hint")).weak().small());
        } else {
            let sp_label = |sp: StartPoint| match sp {
                StartPoint::Empty => fl!(l, "start-empty"),
                StartPoint::Seeded => fl!(l, "start-seeded"),
                StartPoint::Continue => fl!(l, "start-continue"),
            };
            let mut needs_game_shown = false;
            ui.add_enabled_ui(!input.search_running, |ui| {
                for &sp in options {
                    let needs_game = matches!(sp, StartPoint::Seeded | StartPoint::Continue);
                    // Continuing an already-finished game explores nothing.
                    let terminal_block = sp == StartPoint::Continue && input.loaded_terminal;
                    if needs_game && !input.warm_available {
                        needs_game_shown = true;
                    }
                    let enabled = (!needs_game || input.warm_available) && !terminal_block;
                    ui.add_enabled_ui(enabled, |ui| {
                        if ui
                            .selectable_label(input.start_point == sp, sp_label(sp))
                            .clicked()
                        {
                            out.set_start_point = Some(sp);
                        }
                    });
                }
            });
            if needs_game_shown {
                ui.label(RichText::new(fl!(l, "start-needs-game")).weak().small());
            }
            if input.start_point == StartPoint::Continue && input.loaded_terminal {
                ui.label(RichText::new(fl!(l, "start-terminal")).weak().small());
            }
        }
        ui.add_space(8.0);
        if input.search_running {
            ui.horizontal(|ui| {
                if icons::icon_button(ui, Icon::Stop, false, true)
                    .on_hover_text(fl!(l, "btn-stop"))
                    .clicked()
                {
                    out.stop_search = true;
                }
                // Paused → a Play icon resumes; running → a Pause icon.
                let (pi, ptip) = if input.search_paused {
                    (Icon::Play, fl!(l, "btn-resume"))
                } else {
                    (Icon::Pause, fl!(l, "btn-pause"))
                };
                if icons::icon_button(ui, pi, false, true)
                    .on_hover_text(ptip)
                    .clicked()
                {
                    out.toggle_pause = true;
                }
            });
        } else {
            // Can't start with nothing to explore: Perturbation needs a loaded
            // game; Continue from an already-finished position is a no-op.
            let can_start = match input.algo {
                SearchAlgo::Perturbation => input.warm_available,
                _ => !(input.start_point == StartPoint::Continue && input.loaded_terminal),
            };
            if icons::icon_button(ui, Icon::Play, false, can_start)
                .on_hover_text(fl!(l, "btn-start"))
                .clicked()
            {
                out.start_search = true;
            }
        }

        // Checkpoint / resume (systematic + NRPA + perturbation, native only).
        if input.checkpoint_supported
            && matches!(
                input.algo,
                SearchAlgo::Systematic | SearchAlgo::Nrpa | SearchAlgo::Perturbation
            )
        {
            if input.search_running {
                if ui.button(fl!(l, "btn-checkpoint")).clicked() {
                    out.checkpoint = true;
                }
            } else if let Some(ref r) = input.resume {
                if ui.button(fl!(l, "btn-resume-search")).clicked() {
                    out.resume_search = true;
                }
                // Say what Resume will pick up, so it isn't a leap of faith.
                ui.label(
                    RichText::new(format!(
                        "{} · {} · {}",
                        fl!(l, "resume-saved"),
                        algo_label(r.algo),
                        format_dur(r.age),
                    ))
                    .weak()
                    .small(),
                );
            }
        }
        if input.search_running || input.nodes_explored > 0 {
            ui.add_space(6.0);
            ui.label(format!(
                "{} : {}",
                fl!(l, "time-label"),
                format_dur(input.elapsed)
            ));
            ui.label(format!(
                "{} : {}",
                fl!(l, "nodes-explored-label"),
                format_num(input.nodes_explored, sep)
            ));
            if input.search_running && input.nodes_per_sec > 0.0 {
                ui.label(format!(
                    "{} : {}",
                    fl!(l, "nodes-per-second-label"),
                    format_rate(input.nodes_per_sec)
                ));
                // The browser build is markedly slower than native (no OS threads,
                // wasm execution overhead), so its node rate must not be read as a
                // native figure. The factor is machine/browser-dependent — keep it
                // qualitative rather than printing a misleading exact multiplier.
                #[cfg(target_arch = "wasm32")]
                ui.label(
                    RichText::new(fl!(l, "wasm-rate-disclaimer"))
                        .weak()
                        .italics()
                        .small(),
                );
            }
            if !input.records.is_empty() {
                ui.add_space(4.0);
                ui.label(RichText::new(fl!(l, "records-label")).strong());
                for &(score, dur) in input.records.iter().rev().take(5) {
                    ui.label(format!("  {}  {}", score, format_dur(dur)));
                }
            }
            // A finished result preview is on the board: let the user adopt it
            // (becomes the editable game) or dismiss it (back to the played game).
            if input.showing_preview && !input.search_running {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button(fl!(l, "btn-load-best")).clicked() {
                        out.load_best = true;
                    }
                    if ui.button(fl!(l, "btn-dismiss-preview")).clicked() {
                        out.dismiss_preview = true;
                    }
                });
            }
        }
        ui.add_space(10.0);
    }

    out
}

/// The plugin-registry method id for a GUI algorithm (for option-scope filtering).
fn algo_id(a: SearchAlgo) -> &'static str {
    match a {
        SearchAlgo::Nrpa => "nrpa",
        SearchAlgo::Beam => "beam",
        SearchAlgo::Systematic => "systematic",
        SearchAlgo::Perturbation => "perturbation",
    }
}

/// Render the engine-tuning options in scope for `algo`, driven entirely by the plugin
/// registry's [`OptionSpec`](crate::search::plugin::OptionSpec)s. Each widget reads and
/// writes the registry's values map directly (the single source of truth the engine
/// reads at search start), so adding a plugin option needs no change here. Labels and
/// tooltips resolve at runtime via [`crate::i18n::tr`].
fn render_search_options(ui: &mut Ui, algo: SearchAlgo, enabled: bool) {
    use crate::i18n::tr;
    use crate::search::plugin::{registry, OptionKind, OptionValue};
    let reg = registry();
    let id = algo_id(algo);
    let mut shown = false;
    for spec in reg.options() {
        if !spec.scope.applies_to(id) {
            continue;
        }
        if !shown {
            ui.add_space(6.0);
            shown = true;
        }
        let label = tr(spec.label_key);
        let hint = tr(spec.help_key);
        ui.add_enabled_ui(enabled, |ui| match spec.kind {
            OptionKind::Toggle { default } => {
                let mut v = reg.value_bool(spec.key, default);
                if ui.checkbox(&mut v, &label).on_hover_text(&hint).changed() {
                    reg.set_value(spec.key, OptionValue::Toggle(v));
                }
            }
            OptionKind::Float {
                default,
                min,
                max,
                step,
            } => {
                ui.label(RichText::new(&label).strong());
                let mut v = reg.value_f64(spec.key, default);
                if ui
                    .add(egui::Slider::new(&mut v, min..=max).step_by(step))
                    .on_hover_text(&hint)
                    .changed()
                {
                    reg.set_value(spec.key, OptionValue::Float(v));
                }
            }
            OptionKind::Int { default, min, max } => {
                ui.label(RichText::new(&label).strong());
                let mut v = reg.value_int(spec.key, default);
                // A wide range (beam width) is unusable on a linear slider; switch to a
                // logarithmic one past a threshold.
                let slider = egui::Slider::new(&mut v, min..=max);
                let slider = if max - min > 1000 {
                    slider.logarithmic(true)
                } else {
                    slider
                };
                if ui.add(slider).on_hover_text(&hint).changed() {
                    reg.set_value(spec.key, OptionValue::Int(v));
                }
            }
        });
    }
}

fn format_rate(r: f64) -> String {
    if r >= 1_000_000.0 {
        format!("{:.1}M/s", r / 1_000_000.0)
    } else if r >= 1_000.0 {
        format!("{:.1}k/s", r / 1_000.0)
    } else {
        format!("{:.0}/s", r)
    }
}

fn num_sep() -> char {
    // Thousands separator: a comma for English and Japanese; a non-breaking
    // space for the European languages (an SI-acceptable, unambiguous choice
    // that avoids the comma-vs-dot split between e.g. German and French).
    match crate::i18n::current_language().language.as_str() {
        "en" | "ja" => ',',
        _ => '\u{00A0}',
    }
}

fn format_num(n: u64, sep: char) -> String {
    let s = n.to_string();
    if s.len() <= 3 {
        return s;
    }
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            result.push(sep);
        }
        result.push(ch);
    }
    result
}

fn format_dur(d: Duration) -> String {
    let secs = d.as_secs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{}:{:02}", m, s)
    }
}
