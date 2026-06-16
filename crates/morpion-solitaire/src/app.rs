use crate::game::{
    io,
    moves::{legal_moves, Move},
    rules::Variant,
    state::GameState,
};
use crate::i18n::LANGUAGE_LOADER;
use crate::search::{beam, nrpa, systematic, SearchState};
use crate::ui::icons::{self, Icon};
use crate::ui::{board_view, controls};
use controls::{ExportFormat, ResumeInfo, SearchAlgo, StartPoint};
use eframe::egui;
use i18n_embed_fl::fl;
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;
use web_time::Instant;

/// How often a running search auto-checkpoints. A save costs ~10-20 ms
/// (systematic) or under a millisecond (NRPA), so once a minute is negligible.
const AUTO_CHECKPOINT_SECS: f64 = 60.0;

/// NRPA nesting level used when launching/resuming the NRPA search.
const NRPA_LEVEL: usize = 3;

/// The 5T world record (Rosin, 178). Beating it triggers the audio alarm. Only
/// 5T games can exceed this, so the check needs no per-variant logic.
const WORLD_RECORD_5T: u32 = 178;

/// Known record games, loadable from the dropdown (and usable as warm-start
/// seeds). They come from the `morpion-solitaire-records` corpus crate, embedded
/// at compile time as `(display name, .msr record)` pairs.
use morpion_solitaire_records::RECORDS;

/// Install Atkinson Hyperlegible Next as the UI font (both families), so the
/// whole interface uses the legibility-focused typeface. Bundled under the SIL
/// Open Font License (see `assets/fonts/OFL.txt`).
fn install_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions, FontFamily};
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "atkinson".to_owned(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/AtkinsonHyperlegibleNext-Regular.ttf"
        ))),
    );
    // A subset of Noto Sans CJK JP for the Japanese locale (Atkinson is Latin
    // only). It covers the glyphs used in `locales/ja` plus the full kana ranges;
    // adding new Japanese text with unseen kanji means re-subsetting the font.
    fonts.font_data.insert(
        "noto-jp".to_owned(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../assets/fonts/NotoSansJP-subset.otf"
        ))),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        let list = fonts.families.entry(family).or_default();
        list.insert(0, "atkinson".to_owned()); // Latin first
        list.push("noto-jp".to_owned()); // CJK fallback
    }
    ctx.set_fonts(fonts);
}

pub struct MorpionApp {
    state: GameState,
    legal: Vec<Move>,
    hovered: Option<Move>,
    /// Cosmetic display orientation: `view_rot` quarter-turns + optional flip.
    view_rot: u8,
    view_flip: bool,
    /// Draw a direction-talon triangle at each line's origin.
    view_arrows: bool,
    /// Number each played point by its move order.
    view_numbers: bool,
    /// Show the legal-move markers; when off, a move only shows on hover.
    show_legal: bool,
    /// Board zoom factor (mouse wheel) and pan offset (drag), for reading dense
    /// grids up close.
    view_zoom: f32,
    view_pan: egui::Vec2,
    algo: SearchAlgo,
    /// NRPA nesting level. 3 is the fast default (~99 in a minute); 4+ searches
    /// more deeply but only pays off over multi-hour runs.
    nrpa_level: usize,
    /// Where the next search begins (fresh cross, seeded cross, or continue the
    /// loaded position). Coerced to a value the current algorithm supports.
    start_point: StartPoint,
    /// Format for both clipboard copy and file export.
    export_format: ExportFormat,
    selected_variant: Variant,
    search: Option<Arc<SearchState>>,
    search_preview: Option<GameState>,
    search_preview_score: u32,
    /// Number of legal moves available in the current preview position,
    /// cached so it isn't recomputed every frame while the search runs.
    search_preview_legal: usize,
    nodes_per_sec: f64,
    last_rate_nodes: u64,
    last_rate_time: Instant,
    /// When the current search started, and how long it has run. `elapsed` is
    /// updated each frame while running and frozen once the search stops.
    search_start: Instant,
    search_elapsed: Duration,
    /// Last time the systematic search was auto-checkpointed.
    last_checkpoint: Instant,
    import_open: bool,
    /// On narrow (mobile) screens the controls panel is an overlay toggled by a
    /// button; this tracks whether it is open. Ignored on wide screens, where the
    /// panel is always docked. Starts closed so a phone shows the board first.
    controls_open: bool,
    /// How several collinear lines at a point are disambiguated: cursor aim +
    /// scroll wheel (Aim), or click-to-lock + aim + click-to-play (Click). Toggled
    /// from a small overlay on the board.
    input_mode: board_view::InputMode,
    import_text: String,
    status: Option<String>,
    /// Highest record saved per algorithm category ("systematic" | "nrpa" |
    /// "nrpa-seeded" | "beam"); records are kept in per-category subdirectories
    /// and only a strictly higher score in that category is saved. Seeded at
    /// launch from disk.
    saved_best: std::collections::HashMap<&'static str, u32>,
    /// Category of the currently running search (set at start), deciding which
    /// records subdirectory and threshold a found record uses.
    record_category: &'static str,
    /// Whether the "world record beaten" alarm has been triggered this search
    /// (edge guard, so it fires once and silencing sticks).
    alarm_fired: bool,
    /// Shared flag driving the looping audio alarm; cleared to silence it.
    alarm_active: Arc<std::sync::atomic::AtomicBool>,
    /// Human-readable description of the running search's method + parameters,
    /// stored in any record it finds (provenance).
    method_desc: String,
    /// Pre-built, score-aligned labels for the record dropdown (decoded once).
    record_meta: Vec<(Variant, String)>,
    /// Dark vs light UI theme (the board keeps its own light palette).
    dark_mode: bool,
    /// Whether the keyboard-shortcuts help window is open.
    shortcuts_open: bool,
    /// Whether the rules window is open.
    rules_open: bool,
    /// Persisted "don't show the rules on launch again".
    hide_rules: bool,
    /// Whether the hand-played game has unsaved edits (guards destructive actions).
    dirty: bool,
    /// A destructive action awaiting confirmation because the game is dirty.
    pending: Option<PendingAction>,
    /// Editable editorial metadata (author/source/transcribed-by/description/tags)
    /// applied to the user's manual exports and populated from imported records.
    meta: EditorialMeta,
    /// Persisted default author, prefilled into the author field of new games.
    default_author: String,
    /// An export deferred behind the first-time author prompt (set ⇒ prompt open).
    author_prompt: Option<ExportAction>,
    /// Buffer + "remember" toggle for the author prompt.
    author_input: String,
    author_remember: bool,
    /// Whether we have already shown the author prompt this session (don't nag).
    author_asked: bool,
    /// When a systematic search exhausts the whole tree, the (optimal score,
    /// elapsed) to display once in a dialog. Cleared when the dialog is closed.
    exhausted_notice: Option<(u32, Duration)>,
    /// Edge guard so the exhaustion dialog fires once per search.
    exhausted_seen: bool,
}

/// The editorial (free, human-curated) metadata fields the user can edit. Held as
/// plain strings for direct `TextEdit` binding; converted to a [`io::SaveMeta`]
/// (empties → `None`, tags split on commas) at export time.
#[derive(Default, Clone)]
struct EditorialMeta {
    author: String,
    source: String,
    transcribed_by: String,
    description: String,
    /// Comma-separated tags.
    tags: String,
}

/// A manual export the user asked for, possibly deferred behind the author prompt.
/// `File` saves to a file on native (a save dialog) and downloads in the browser.
#[derive(Clone, Copy)]
enum ExportAction {
    Copy,
    File,
}

/// A board-replacing action deferred behind the unsaved-changes confirmation.
#[derive(Clone, Copy)]
enum PendingAction {
    NewGame(Variant),
    LoadMsr(&'static str),
}

/// Per-record metadata for the dropdown, aligned 1:1 with `RECORDS`: the record's
/// variant (so the list can be filtered to the selected one) and a score-aligned
/// label (the move count right-aligned and first, so the numbers line up).
fn record_meta() -> Vec<(Variant, String)> {
    RECORDS
        .iter()
        .map(|(name, _id, msr)| match io::import_save(msr) {
            Ok(st) => {
                let score = st.score();
                let creator = name
                    .replacen(&score.to_string(), "", 1)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                (st.variant, format!("{score:>3}  {creator}"))
            }
            Err(_) => (Variant::T5, name.to_string()),
        })
        .collect()
}

impl MorpionApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        // Restore persisted UI preferences (see `save`). Everything here is a
        // setting, not game state; an unknown/absent value falls back to default.
        let get = |k: &str| cc.storage.and_then(|s| s.get_string(k));
        let get_bool = |k: &str, default: bool| get(k).map(|v| v == "true").unwrap_or(default);
        let dark_mode = get_bool("dark_mode", true);
        let hide_rules = get_bool("hide_rules", false);
        let default_author = get("default_author").unwrap_or_default();
        let view_arrows = get_bool("view_arrows", true);
        let view_numbers = get_bool("view_numbers", true);
        let show_legal = get_bool("show_legal", true);
        // Enums persist as JSON; each parse is its own expression so the target
        // type is inferred independently from the fallback.
        let input_mode = get("input_mode")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(board_view::InputMode::Aim);
        let algo = get("algo")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(SearchAlgo::Nrpa);
        let start_point = get("start_point")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(StartPoint::Empty);
        let export_format = get("export_format")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(ExportFormat::Msr);
        let nrpa_level = get("nrpa_level")
            .and_then(|s| s.parse().ok())
            .unwrap_or(NRPA_LEVEL);
        let variant = get("variant")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Variant::T5);
        cc.egui_ctx.set_visuals(if dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        // The UI language is set by the platform entry point (native `run_native`
        // or the wasm shell) before the app is built, so this stays free of any
        // OS/browser locale API.
        let state = GameState::new(variant);
        let legal = legal_moves(&state);
        Self {
            state,
            legal,
            hovered: None,
            view_rot: 0,
            view_flip: false,
            view_arrows,
            view_numbers,
            show_legal,
            view_zoom: 1.0,
            view_pan: egui::Vec2::ZERO,
            algo,
            nrpa_level,
            start_point,
            export_format,
            selected_variant: variant,
            search: None,
            search_preview: None,
            search_preview_score: 0,
            search_preview_legal: 0,
            nodes_per_sec: 0.0,
            last_rate_nodes: 0,
            last_rate_time: Instant::now(),
            search_start: Instant::now(),
            search_elapsed: Duration::ZERO,
            last_checkpoint: Instant::now(),
            import_open: false,
            controls_open: false,
            input_mode,
            import_text: String::new(),
            status: None,
            // Seed each category's threshold from disk so we only persist a
            // position that genuinely beats what that algorithm already saved.
            saved_best: load_saved_bests(),
            record_category: "nrpa",
            method_desc: String::new(),
            alarm_fired: false,
            alarm_active: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            record_meta: record_meta(),
            dark_mode,
            shortcuts_open: false,
            rules_open: !hide_rules,
            hide_rules,
            dirty: false,
            pending: None,
            meta: EditorialMeta {
                author: default_author.clone(),
                ..Default::default()
            },
            default_author,
            author_prompt: None,
            author_input: String::new(),
            author_remember: true,
            author_asked: false,
            exhausted_notice: None,
            exhausted_seen: false,
        }
    }

    /// Run a board-replacing action now, or defer it behind a confirmation when
    /// the hand-played game has unsaved edits.
    fn request(&mut self, action: PendingAction) {
        if self.dirty {
            self.pending = Some(action);
        } else {
            self.perform(action);
        }
    }

    fn perform(&mut self, action: PendingAction) {
        match action {
            PendingAction::NewGame(v) => {
                self.selected_variant = v;
                self.new_game();
            }
            PendingAction::LoadMsr(msr) => self.try_import(msr),
        }
    }

    /// The position currently shown to the user — the search preview when one
    /// exists (read-only result), otherwise the played state (editable).
    fn displayed_state(&self) -> &GameState {
        self.search_preview.as_ref().unwrap_or(&self.state)
    }

    fn rotate_view(&mut self) {
        self.view_rot = (self.view_rot + 1) % 4;
    }
    fn flip_view(&mut self) {
        self.view_flip = !self.view_flip;
    }
    /// Reset zoom and pan so the board fits the view again.
    fn recenter_view(&mut self) {
        self.view_zoom = 1.0;
        self.view_pan = egui::Vec2::ZERO;
    }

    /// Icon toolbars overlaid on the board: view/display controls (top-left) and
    /// undo/redo (top-right). Editing is disabled while a search preview is shown.
    fn board_toolbars(&mut self, ctx: &egui::Context, board_rect: egui::Rect) {
        let l = &*LANGUAGE_LOADER;
        let editable = self.search_preview.is_none();
        let frame = |ui: &egui::Ui| egui::Frame::popup(ui.style());

        // Anchor offsets are relative to the screen edges, so derive them from the
        // board rect: the left toolbar hugs the board's top-left, the right one the
        // board's top-right (which is the controls panel's left edge, not the
        // window edge).
        let screen = ctx.content_rect();
        let left_off = egui::vec2(board_rect.left() + 10.0, board_rect.top() + 10.0);
        let right_off = egui::vec2(
            board_rect.right() - screen.right() - 10.0,
            board_rect.top() + 10.0,
        );

        egui::Area::new(egui::Id::new("board_toolbar_view"))
            .anchor(egui::Align2::LEFT_TOP, left_off)
            .show(ctx, |ui| {
                frame(ui).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if icons::icon_button(ui, Icon::Rotate, false, true)
                            .on_hover_text(format!("{} (R)", fl!(l, "btn-rotate")))
                            .clicked()
                        {
                            self.rotate_view();
                        }
                        if icons::icon_button(ui, Icon::Flip, false, true)
                            .on_hover_text(format!("{} (F)", fl!(l, "btn-flip")))
                            .clicked()
                        {
                            self.flip_view();
                        }
                        if icons::icon_button(ui, Icon::Recenter, false, true)
                            .on_hover_text(format!("{} (G)", fl!(l, "btn-recenter")))
                            .clicked()
                        {
                            self.recenter_view();
                        }
                        ui.separator();
                        if icons::icon_button(ui, Icon::Arrows, self.view_arrows, true)
                            .on_hover_text(fl!(l, "btn-arrows"))
                            .clicked()
                        {
                            self.view_arrows = !self.view_arrows;
                        }
                        if icons::icon_button(ui, Icon::Numbers, self.view_numbers, true)
                            .on_hover_text(fl!(l, "btn-numbers"))
                            .clicked()
                        {
                            self.view_numbers = !self.view_numbers;
                        }
                        if icons::icon_button(ui, Icon::Targets, self.show_legal, true)
                            .on_hover_text(fl!(l, "legal-moves-label"))
                            .clicked()
                        {
                            self.show_legal = !self.show_legal;
                        }
                    });
                });
            });

        egui::Area::new(egui::Id::new("board_toolbar_edit"))
            .anchor(egui::Align2::RIGHT_TOP, right_off)
            .show(ctx, |ui| {
                frame(ui).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if icons::icon_button(
                            ui,
                            Icon::Undo,
                            false,
                            editable && self.state.can_undo(),
                        )
                        .on_hover_text(format!(
                            "{} ({}Z)",
                            fl!(l, "btn-undo"),
                            crate::ui::cmd_key()
                        ))
                        .clicked()
                        {
                            self.state.undo();
                            self.refresh_legal();
                            self.dirty = true;
                        }
                        if icons::icon_button(
                            ui,
                            Icon::Redo,
                            false,
                            editable && self.state.can_redo(),
                        )
                        .on_hover_text(format!(
                            "{} ({}R)",
                            fl!(l, "btn-redo"),
                            crate::ui::cmd_key()
                        ))
                        .clicked()
                        {
                            self.state.redo();
                            self.refresh_legal();
                            self.dirty = true;
                        }
                    });
                });
            });

        // Line-picker mode toggle, centred on the board's bottom edge: Aim (cursor
        // + scroll wheel) vs Click (click to lock, aim, click to play).
        let mode_off = egui::vec2(
            board_rect.center().x - screen.center().x,
            board_rect.bottom() - screen.bottom() - 10.0,
        );
        egui::Area::new(egui::Id::new("board_picker_mode"))
            .anchor(egui::Align2::CENTER_BOTTOM, mode_off)
            .show(ctx, |ui| {
                frame(ui).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        use board_view::InputMode;
                        ui.label(fl!(l, "pick-mode-label"));
                        if ui
                            .selectable_label(
                                self.input_mode == InputMode::Aim,
                                fl!(l, "pick-mode-aim"),
                            )
                            .on_hover_text(fl!(l, "pick-mode-aim-hint"))
                            .clicked()
                        {
                            self.input_mode = InputMode::Aim;
                        }
                        if ui
                            .selectable_label(
                                self.input_mode == InputMode::Click,
                                fl!(l, "pick-mode-click"),
                            )
                            .on_hover_text(fl!(l, "pick-mode-click-hint"))
                            .clicked()
                        {
                            self.input_mode = InputMode::Click;
                        }
                    });
                });
            });
    }

    /// Copy the currently displayed position to the clipboard as JSON.
    /// Render options for image export: match the on-board "numbers" toggle so the
    /// exported picture looks like what's shown.
    fn render_opts(&self) -> crate::render::RenderOpts {
        crate::render::RenderOpts {
            numbers: self.view_numbers,
        }
    }

    /// The user's editorial metadata as an [`io::SaveMeta`]: blank fields become
    /// `None`, tags split on commas. No solver fields — these are manual exports.
    fn build_meta(&self) -> io::SaveMeta {
        let opt = |s: &str| {
            let t = s.trim();
            (!t.is_empty()).then(|| t.to_owned())
        };
        io::SaveMeta {
            description: opt(&self.meta.description),
            author: opt(&self.meta.author),
            source: opt(&self.meta.source),
            transcribed_by: opt(&self.meta.transcribed_by),
            tags: self
                .meta
                .tags
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect(),
            ..Default::default()
        }
    }

    /// Serialise the displayed position to text in one of the text formats.
    fn export_text(&self, fmt: ExportFormat) -> Result<String, String> {
        let st = self.displayed_state();
        match fmt {
            ExportFormat::Msr => io::export_save_with_meta(st, io::unix_now(), &self.build_meta())
                .map_err(|e| e.to_string()),
            ExportFormat::Json => io::export_json_with_meta(st, io::unix_now(), &self.build_meta())
                .map_err(|e| e.to_string()),
            ExportFormat::Pentasol => Ok(io::export_pentasol(st)),
            // The SVG carries the record in a <metadata> element, so the picture
            // is also a loadable save (and survives a clipboard text copy).
            ExportFormat::Svg => {
                let msr = self.export_text(ExportFormat::Msr)?;
                Ok(crate::render::embed_msr_svg(
                    &crate::render::to_svg(st, &self.render_opts()),
                    &msr,
                ))
            }
            // PNG is binary; it never goes through the text path.
            ExportFormat::Png => Err("PNG is not a text format".to_owned()),
        }
    }

    /// Copy the displayed position to the clipboard in the selected format. Text
    /// formats go as text; PNG goes as an image (native only — the web build has
    /// no rasteriser, so it reports that).
    fn copy_to_clipboard(&mut self, ctx: &egui::Context) {
        let l = &*LANGUAGE_LOADER;
        if self.export_format == ExportFormat::Png {
            #[cfg(not(target_arch = "wasm32"))]
            {
                // The clipboard image is raw pixels, so unlike a PNG *file* it
                // can't carry the embedded record — warn that it's picture-only.
                self.status = Some(match self.copy_png_image() {
                    Ok(()) => fl!(l, "status-copied-png-no-record"),
                    Err(e) => fl!(l, "status-import-error", error = e),
                });
            }
            #[cfg(target_arch = "wasm32")]
            {
                self.status = Some(fl!(l, "status-png-web"));
            }
            return;
        }
        match self.export_text(self.export_format) {
            Ok(text) => {
                ctx.copy_text(text);
                self.status = Some(fl!(l, "status-copied"));
            }
            Err(e) => self.status = Some(fl!(l, "status-import-error", error = e)),
        }
    }

    /// Put the rendered board on the clipboard as an image. Native only.
    #[cfg(not(target_arch = "wasm32"))]
    fn copy_png_image(&self) -> Result<(), String> {
        let (width, height, bytes) =
            crate::render::to_rgba(self.displayed_state(), &self.render_opts())?;
        let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        clipboard
            .set_image(arboard::ImageData {
                width,
                height,
                bytes: std::borrow::Cow::Owned(bytes),
            })
            .map_err(|e| e.to_string())
    }

    /// Export the displayed position to a file. The save dialog offers every
    /// format as a file-type filter (defaulting to the side-panel selection), and
    /// the actual format follows the extension the user picks. Returns whether a
    /// file was actually written (false if the dialog was cancelled or it failed).
    /// Native only.
    #[cfg(not(target_arch = "wasm32"))]
    fn export_to_file(&mut self) -> bool {
        let l = &*LANGUAGE_LOADER;
        let ext_of = |f| match f {
            ExportFormat::Msr => "msr",
            ExportFormat::Json => "json",
            ExportFormat::Pentasol => "psol",
            ExportFormat::Svg => "svg",
            ExportFormat::Png => "png",
        };
        // Default to the selected format, but list every format as a filter so the
        // user can switch it right in the dialog.
        let sel = self.export_format;
        let order = std::iter::once(sel).chain(
            controls::export_formats()
                .iter()
                .copied()
                .filter(|&f| f != sel),
        );
        let mut dialog = rfd::FileDialog::new().set_file_name(format!(
            "morpion-{}.{}",
            self.displayed_state().score(),
            ext_of(sel)
        ));
        for f in order {
            dialog = dialog.add_filter(controls::export_format_label(f), &[ext_of(f)]);
        }
        let Some(mut path) = dialog.save_file() else {
            return false;
        };
        // The chosen extension decides the format (falling back to the selection),
        // and we make sure the file actually carries that extension.
        let fmt = path
            .extension()
            .and_then(|e| e.to_str())
            .and_then(|e| match e.to_ascii_lowercase().as_str() {
                "msr" => Some(ExportFormat::Msr),
                "json" => Some(ExportFormat::Json),
                "psol" => Some(ExportFormat::Pentasol),
                "svg" => Some(ExportFormat::Svg),
                "png" => Some(ExportFormat::Png),
                _ => None,
            })
            .unwrap_or(sel);
        if path.extension().is_none() {
            path.set_extension(ext_of(fmt));
        }
        let bytes = match fmt {
            // Embed the record in the PNG (tEXt chunk) so the picture is a save too.
            ExportFormat::Png => {
                let msr = self.export_text(ExportFormat::Msr);
                crate::render::to_png(self.displayed_state(), &self.render_opts())
                    .and_then(|png| Ok(crate::render::embed_msr_png(&png, &msr?)))
            }
            other => self.export_text(other).map(String::into_bytes),
        };
        let bytes = match bytes {
            Ok(b) => b,
            Err(e) => {
                self.status = Some(fl!(l, "status-import-error", error = e));
                return false;
            }
        };
        match std::fs::write(&path, &bytes) {
            Ok(()) => {
                self.dirty = false;
                self.status = Some(fl!(l, "status-exported", path = path.display().to_string()));
                true
            }
            Err(e) => {
                self.status = Some(fl!(l, "status-import-error", error = e.to_string()));
                false
            }
        }
    }

    /// Handle a copy/export request. The first time the user exports without an
    /// author set, pop a one-time prompt to collect (and optionally remember) their
    /// name; otherwise export straight away.
    fn request_export(&mut self, action: ExportAction, ctx: &egui::Context) {
        if self.meta.author.trim().is_empty() && !self.author_asked {
            self.author_input = self.default_author.clone();
            self.author_remember = true;
            self.author_prompt = Some(action);
        } else {
            self.perform_export(action, ctx);
        }
    }

    /// Actually run a deferred export.
    fn perform_export(&mut self, action: ExportAction, ctx: &egui::Context) {
        match action {
            ExportAction::Copy => self.copy_to_clipboard(ctx),
            ExportAction::File => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.export_to_file();
                }
                #[cfg(target_arch = "wasm32")]
                {
                    let _ = ctx;
                    self.download_to_browser();
                }
            }
        }
    }

    /// Save the current export to a file the browser downloads (the web build's
    /// equivalent of the native save dialog). Binary PNG has no wasm rasteriser,
    /// so it stays native-only; everything else is text.
    #[cfg(target_arch = "wasm32")]
    fn download_to_browser(&mut self) {
        let l = &*LANGUAGE_LOADER;
        if self.export_format == ExportFormat::Png {
            self.status = Some(fl!(l, "status-png-web"));
            return;
        }
        let ext = match self.export_format {
            ExportFormat::Msr => "msr",
            ExportFormat::Json => "json",
            ExportFormat::Pentasol => "psol",
            ExportFormat::Svg => "svg",
            ExportFormat::Png => "png", // unreachable (handled above)
        };
        let name = format!("morpion-{}.{ext}", self.displayed_state().score());
        match self.export_text(self.export_format) {
            Ok(text) => {
                self.status = Some(match browser_download(&name, text.as_bytes()) {
                    Ok(()) => fl!(l, "status-exported", path = name),
                    Err(e) => fl!(l, "status-import-error", error = e),
                });
            }
            Err(e) => self.status = Some(fl!(l, "status-import-error", error = e)),
        }
    }

    /// The first-export "your name" prompt. On confirm it fills the author field
    /// (optionally persisting it as the default), then runs the pending export.
    fn author_prompt_window(&mut self, ctx: &egui::Context) {
        let Some(action) = self.author_prompt else {
            return;
        };
        let l = &*LANGUAGE_LOADER;
        let mut decided: Option<bool> = None; // Some(true)=save, Some(false)=skip
        egui::Window::new(fl!(l, "author-prompt-title"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_max_width(360.0);
                ui.label(fl!(l, "author-prompt-body"));
                ui.add_space(8.0);
                let entered = ui
                    .add(
                        egui::TextEdit::singleline(&mut self.author_input)
                            .desired_width(f32::INFINITY)
                            .hint_text(fl!(l, "meta-author")),
                    )
                    .lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.add_space(4.0);
                ui.checkbox(&mut self.author_remember, fl!(l, "author-prompt-remember"));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(fl!(l, "author-prompt-ok")).clicked() || entered {
                        decided = Some(true);
                    }
                    if ui.button(fl!(l, "author-prompt-skip")).clicked() {
                        decided = Some(false);
                    }
                });
            });
        if let Some(save) = decided {
            self.author_asked = true;
            self.author_prompt = None;
            if save {
                let name = self.author_input.trim().to_owned();
                if !name.is_empty() {
                    self.meta.author = name.clone();
                    if self.author_remember {
                        self.default_author = name;
                    }
                }
            }
            self.perform_export(action, ctx);
        }
    }

    /// A floating spinner + node-rate readout over the board while a search runs,
    /// so progress is visible without watching the side panel.
    fn search_overlay(&self, ctx: &egui::Context, board_rect: egui::Rect) {
        if !self.search_running() {
            return;
        }
        let l = &*LANGUAGE_LOADER;
        let rate = self.nodes_per_sec;
        let rate_txt = if rate >= 1e6 {
            format!("{:.1}M/s", rate / 1e6)
        } else if rate >= 1e3 {
            format!("{:.0}k/s", rate / 1e3)
        } else {
            format!("{rate:.0}/s")
        };
        egui::Area::new(egui::Id::new("search_overlay"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(
                board_rect.center().x - 70.0,
                board_rect.top() + 8.0,
            ))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().size(16.0));
                        ui.label(format!("{} · {}", fl!(l, "searching-label"), rate_txt));
                    });
                });
            });
    }

    /// A keyboard-shortcuts cheat sheet window (toggled by the "?" button).
    fn shortcuts_window(&mut self, ctx: &egui::Context) {
        if !self.shortcuts_open {
            return;
        }
        let l = &*LANGUAGE_LOADER;
        let cmd = crate::ui::cmd_key();
        let mut open = self.shortcuts_open;
        egui::Window::new(fl!(l, "shortcuts-title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                egui::Grid::new("shortcuts_grid")
                    .num_columns(2)
                    .spacing([18.0, 6.0])
                    .show(ui, |ui| {
                        let rows = [
                            ("R".to_owned(), fl!(l, "btn-rotate")),
                            ("F".to_owned(), fl!(l, "btn-flip")),
                            ("G".to_owned(), fl!(l, "btn-recenter")),
                            (format!("{cmd}Z"), fl!(l, "btn-undo")),
                            (format!("{cmd}R"), fl!(l, "btn-redo")),
                            (format!("{cmd}N"), fl!(l, "btn-new")),
                            (format!("{cmd}S"), fl!(l, "btn-export-file")),
                            (format!("{cmd}C"), fl!(l, "btn-copy")),
                            (format!("{cmd}V"), fl!(l, "btn-import")),
                        ];
                        for (key, action) in rows {
                            ui.label(egui::RichText::new(key).monospace().strong());
                            ui.label(action);
                            ui.end_row();
                        }
                    });
            });
        self.shortcuts_open = open;
    }

    /// Announce that the systematic search has exhausted the whole game tree, so
    /// its best score is the proven optimum. Shown once, dismissed with a button.
    fn exhausted_window(&mut self, ctx: &egui::Context) {
        let Some((score, elapsed)) = self.exhausted_notice else {
            return;
        };
        let l = &*LANGUAGE_LOADER;
        let secs = elapsed.as_secs();
        let time = format!("{}:{:02}", secs / 60, secs % 60);
        let score = score as i64;
        let mut close = false;
        egui::Window::new(fl!(l, "exhausted-title"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_max_width(380.0);
                ui.label(fl!(l, "exhausted-body", time = time, score = score));
                ui.add_space(10.0);
                ui.vertical_centered(|ui| {
                    if ui.button(fl!(l, "btn-close")).clicked() {
                        close = true;
                    }
                });
            });
        if close {
            self.exhausted_notice = None;
        }
    }

    /// The rules window — shown on first launch (unless dismissed) and reopenable
    /// via the info button. A "don't show again" checkbox persists the choice.
    fn rules_window(&mut self, ctx: &egui::Context) {
        if !self.rules_open {
            return;
        }
        let l = &*LANGUAGE_LOADER;
        let mut open = true;
        egui::Window::new(fl!(l, "rules-title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_max_width(440.0);
                ui.label(fl!(l, "rules-body"));
                ui.add_space(10.0);
                ui.checkbox(&mut self.hide_rules, fl!(l, "rules-hide"));
                ui.add_space(4.0);
                if ui.button(fl!(l, "btn-close")).clicked() {
                    self.rules_open = false;
                }
            });
        if !open {
            self.rules_open = false;
        }
    }

    /// Confirmation for a deferred board-replacing action when the game has
    /// unsaved edits — the standard Save / Don't save / Cancel choice.
    fn confirm_pending(&mut self, ctx: &egui::Context) {
        if self.pending.is_none() {
            return;
        }
        let l = &*LANGUAGE_LOADER;
        #[derive(Clone, Copy)]
        enum Choice {
            Save,
            DontSave,
            Cancel,
        }
        let mut choice = None;
        egui::Window::new(fl!(l, "confirm-discard-title"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_max_width(300.0);
                ui.label(fl!(l, "confirm-discard-body"));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button(fl!(l, "btn-save")).clicked() {
                        choice = Some(Choice::Save);
                    }
                    if ui.button(fl!(l, "btn-dont-save")).clicked() {
                        choice = Some(Choice::DontSave);
                    }
                    if ui.button(fl!(l, "btn-cancel")).clicked() {
                        choice = Some(Choice::Cancel);
                    }
                });
            });
        match choice {
            // Save first; only proceed once the game is actually saved, so a
            // cancelled save dialog leaves this confirmation open.
            Some(Choice::Save) => {
                #[cfg(not(target_arch = "wasm32"))]
                let saved = self.export_to_file();
                #[cfg(target_arch = "wasm32")]
                let saved = {
                    self.copy_to_clipboard(ctx);
                    true
                };
                if saved {
                    if let Some(action) = self.pending.take() {
                        self.perform(action);
                    }
                }
            }
            Some(Choice::DontSave) => {
                if let Some(action) = self.pending.take() {
                    self.dirty = false;
                    self.perform(action);
                }
            }
            Some(Choice::Cancel) => self.pending = None,
            None => {}
        }
    }

    /// Try to load a saved game from arbitrary text (JSON first, then Pentasol).
    fn try_import(&mut self, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        let result = io::import_save_with_info(trimmed)
            .map(|(state, info)| (state, Some(info)))
            .or_else(|_| {
                io::import_pentasol(trimmed, self.selected_variant).map(|state| (state, None))
            });
        match result {
            Ok((state, info)) => {
                self.stop_search();
                self.search_preview = None;
                self.search_preview_score = 0;
                self.selected_variant = state.variant;
                self.state = state;
                // Surface the record's editorial metadata in the editor (Pentasol
                // carries none, so leave the fields as they are for that path).
                if let Some(info) = info {
                    self.meta = EditorialMeta {
                        author: info.author.unwrap_or_default(),
                        source: info.source.unwrap_or_default(),
                        transcribed_by: info.transcribed_by.unwrap_or_default(),
                        description: info.description.unwrap_or_default(),
                        tags: info.tags.join(", "),
                    };
                }
                self.refresh_legal();
                self.dirty = false;
                self.import_open = false;
                self.import_text.clear();
                self.status = Some(fl!(
                    LANGUAGE_LOADER,
                    "status-imported",
                    score = (self.state.score() as i64)
                ));
            }
            Err(e) => {
                self.status = Some(fl!(LANGUAGE_LOADER, "status-import-error", error = e));
            }
        }
    }

    /// Load a dropped file: a `.msr`/`.json`/`.psol` text save, or a `.png`/`.svg`
    /// that carries an embedded record. When an image has no embedded record (a
    /// foreign picture, or one re-encoded by an editor that dropped the chunk),
    /// say so plainly rather than failing with a parse error.
    fn load_file_bytes(&mut self, name: &str, bytes: &[u8]) {
        let l = &*LANGUAGE_LOADER;
        let lower = name.to_ascii_lowercase();
        let text = std::str::from_utf8(bytes).ok();
        let is_png = lower.ends_with(".png") || bytes.starts_with(b"\x89PNG\r\n\x1a\n");
        let is_svg = lower.ends_with(".svg") || text.is_some_and(|t| t.contains("<svg"));

        if is_png {
            match crate::render::extract_msr_png(bytes) {
                Some(msr) => self.try_import(&msr),
                None => self.status = Some(fl!(l, "status-no-msr-data")),
            }
        } else if is_svg {
            match text.and_then(crate::render::extract_msr_svg) {
                Some(msr) => self.try_import(&msr),
                None => self.status = Some(fl!(l, "status-no-msr-data")),
            }
        } else if let Some(text) = text {
            self.try_import(text);
        } else {
            self.status = Some(fl!(l, "status-no-msr-data"));
        }
    }

    /// Pull any files dropped on the window this frame and load the first one,
    /// and show a hint while files hover over the window.
    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let center = ctx.content_rect().center();
            egui::Area::new(egui::Id::new("drop_overlay"))
                .order(egui::Order::Foreground)
                .fixed_pos(center - egui::vec2(160.0, 16.0))
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(fl!(LANGUAGE_LOADER, "drop-hint"));
                    });
                });
        }
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for f in dropped {
            if let Some(bytes) = &f.bytes {
                self.load_file_bytes(&f.name, bytes);
                break;
            }
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(path) = &f.path {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_owned();
                match std::fs::read(path) {
                    Ok(b) => self.load_file_bytes(&name, &b),
                    Err(e) => {
                        self.status = Some(fl!(
                            LANGUAGE_LOADER,
                            "status-import-error",
                            error = e.to_string()
                        ))
                    }
                }
                break;
            }
        }
    }

    /// Persist a record-breaking position to disk (native) so it is never lost.
    fn persist_record(&mut self, score: u32) {
        let Some(preview) = self.search_preview.clone() else {
            return;
        };
        let epoch = io::unix_now();
        let method = (!self.method_desc.is_empty()).then(|| self.method_desc.clone());
        let json = match io::export_save_with_method(&preview, epoch, method) {
            Ok(j) => j,
            Err(e) => {
                self.status = Some(fl!(
                    LANGUAGE_LOADER,
                    "status-record-save-error",
                    error = e.to_string()
                ));
                return;
            }
        };
        #[cfg(not(target_arch = "wasm32"))]
        match save_record_file(&json, score, self.record_category) {
            Ok(path) => {
                let p = path.display().to_string();
                log::info!("record {score} saved to {p}");
                self.status = Some(fl!(
                    LANGUAGE_LOADER,
                    "status-record-saved",
                    score = (score as i64),
                    path = p
                ));
            }
            Err(e) => {
                log::error!("failed to save record {score}: {e}");
                self.status = Some(fl!(
                    LANGUAGE_LOADER,
                    "status-record-save-error",
                    error = e.to_string()
                ));
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            // No filesystem in the browser: keep the position in the clipboard
            // as a best-effort fallback so it can still be saved by the user.
            let _ = &json;
            log::info!("record {score} reached (web build, no disk save)");
            self.status = Some(fl!(
                LANGUAGE_LOADER,
                "status-record-web",
                score = (score as i64)
            ));
        }
    }

    /// A search tried to place a point off the fixed grid. Rather than crash,
    /// stop the search, save the best game reached (so a long game isn't lost),
    /// and alert. The grid can then be enlarged by widening `Row` in `board.rs`
    /// (e.g. `u128` → a wider word) and the search resumed from the checkpoint.
    fn handle_grid_overflow(&mut self, score: u32) {
        self.stop_search();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(preview) = self.search_preview.clone() {
            let epoch = io::unix_now();
            let note = format!("grid overflow {g}x{g}", g = crate::game::board::GRID);
            if let Ok(json) = io::export_save_with_method(&preview, epoch, Some(note)) {
                match save_record_file(&json, score, "overflow") {
                    Ok(path) => {
                        log::warn!("grid overflow: best {score} saved to {}", path.display())
                    }
                    Err(e) => log::error!("grid overflow: save failed: {e}"),
                }
            }
        }
        self.status = Some(fl!(
            LANGUAGE_LOADER,
            "status-overflow",
            grid = (crate::game::board::GRID as i64),
            score = (score as i64)
        ));
    }

    fn new_game(&mut self) {
        self.stop_search();
        self.search_preview = None;
        self.search_preview_score = 0;
        self.state = GameState::new(self.selected_variant);
        self.refresh_legal();
        self.dirty = false;
        // A fresh game inherits no provenance from a previously loaded record;
        // keep only the player's author (the one field that carries over).
        self.meta = EditorialMeta {
            author: self.meta.author.clone(),
            ..Default::default()
        };
    }

    fn refresh_legal(&mut self) {
        self.legal = legal_moves(&self.state);
        self.hovered = None;
    }

    fn start_search(&mut self) {
        self.stop_search();
        self.alarm_fired = false;
        self.alarm_active.store(false, Ordering::Relaxed);
        self.search_preview = None;
        self.search_preview_score = 0;
        self.search_start = Instant::now();
        self.search_elapsed = Duration::ZERO;
        self.last_checkpoint = Instant::now();
        self.exhausted_seen = false;
        let s = SearchState::new();
        let s2 = s.clone();
        let algo = self.algo;
        let level = self.nrpa_level;
        // Seeded NRPA starts from a fresh cross but seeds the policy from the
        // loaded game's moves (the loaded game is a prior, not the start).
        let warm_seq = (algo == SearchAlgo::Nrpa
            && self.start_point == StartPoint::Seeded
            && !self.state.history.is_empty())
        .then(|| self.state.history.clone());
        // Continue searches from the loaded position; Empty/Seeded from a fresh
        // cross. The fresh cross must use the seed game's variant when seeding, so
        // the seed sequence stays legal on it.
        let from_fresh = self.start_point != StartPoint::Continue;
        let variant = if warm_seq.is_some() {
            self.state.variant
        } else {
            self.selected_variant
        };
        let initial = if from_fresh {
            GameState::new(variant)
        } else {
            self.state.clone()
        };

        // Classify this search for record provenance and per-category saving.
        let seeded = warm_seq.is_some() || algo == SearchAlgo::Perturbation;
        let seed_len = self.state.history.len();
        self.record_category = match algo {
            SearchAlgo::Systematic => "systematic",
            SearchAlgo::Beam => "beam",
            SearchAlgo::Nrpa | SearchAlgo::Perturbation if seeded => "nrpa-seeded",
            SearchAlgo::Nrpa | SearchAlgo::Perturbation => "nrpa",
        };
        self.method_desc = match algo {
            SearchAlgo::Systematic => "systematic (exhaustive)".to_owned(),
            SearchAlgo::Beam => "beam".to_owned(),
            SearchAlgo::Perturbation => {
                format!(
                    "perturbation L{level} warm-from={seed_len} warm={}",
                    nrpa::WARM_ITERS
                )
            }
            SearchAlgo::Nrpa if seeded => {
                format!(
                    "nrpa-seeded L{level} warm-from={seed_len} warm={}",
                    nrpa::WARM_ITERS
                )
            }
            SearchAlgo::Nrpa => format!("nrpa L{level}"),
        };
        // Seeded searches must not re-save the seed: only games beating it count.
        if seeded {
            let floor = self.saved_best.entry(self.record_category).or_insert(0);
            *floor = (*floor).max(seed_len as u32);
        }

        // Perturbation runs an outer loop driving time-bounded inner searches via
        // OS threads (native only); seed it from the loaded game.
        #[cfg(not(target_arch = "wasm32"))]
        if algo == SearchAlgo::Perturbation {
            let seed = self.state.history.clone();
            let pvariant = self.state.variant;
            std::thread::spawn(move || nrpa::run_perturbation(s2, level, seed, pvariant));
            self.search = Some(s);
            return;
        }
        // Show the position the search actually starts from right away, so a
        // previously loaded record doesn't linger on the board until the first
        // improvement arrives (a fresh cross for warm/from-new, else the seed).
        self.search_preview = Some(initial.clone());

        rayon::spawn(move || match algo {
            SearchAlgo::Systematic => systematic::run(&initial, s2),
            SearchAlgo::Nrpa => match warm_seq {
                Some(seq) => nrpa::run_warm(&initial, s2, level, &seq, nrpa::WARM_ITERS),
                None => nrpa::run(&initial, s2, level),
            },
            SearchAlgo::Beam => beam::run(&initial, s2, 64),
            SearchAlgo::Perturbation => {} // native: handled above; wasm: not offered
        });
        self.search = Some(s);
    }

    /// Resume the saved search from its checkpoint (native only). Dispatches to
    /// the engine that produced it, per the checkpoint's `algo` tag.
    #[cfg(not(target_arch = "wasm32"))]
    fn resume_search(&mut self) {
        let Some(cp) = crate::search::checkpoint::load(algo_tag(self.algo)) else {
            self.status = Some(fl!(LANGUAGE_LOADER, "status-no-checkpoint"));
            return;
        };
        self.stop_search();
        self.selected_variant = cp.variant;
        // Don't leave a previously loaded record on the board while the resumed
        // search spins up; show a fresh cross of the checkpoint's variant.
        self.search_preview = Some(GameState::new(cp.variant));
        self.search_preview_score = 0;
        self.search_start = Instant::now();
        self.search_elapsed = Duration::ZERO;
        self.last_checkpoint = Instant::now();
        self.exhausted_seen = false;
        let s = SearchState::new();
        let s2 = s.clone();
        let level = self.nrpa_level;
        let variant = cp.variant;
        match cp.algo.as_str() {
            "perturbation" => {
                self.algo = SearchAlgo::Perturbation;
                let archive = cp.frontier; // the saved population
                std::thread::spawn(move || nrpa::resume_perturbation(s2, level, variant, archive));
            }
            "nrpa" => {
                self.algo = SearchAlgo::Nrpa;
                rayon::spawn(move || nrpa::resume(s2, cp, level));
            }
            _ => {
                self.algo = SearchAlgo::Systematic;
                rayon::spawn(move || systematic::resume(s2, cp));
            }
        }
        self.search = Some(s);
        self.status = Some(fl!(LANGUAGE_LOADER, "status-resumed"));
    }

    #[cfg(target_arch = "wasm32")]
    fn resume_search(&mut self) {}

    /// Write a checkpoint for the running search. Systematic checkpointing is
    /// done inside the worker loop (it must snapshot a stable frontier), so we
    /// just raise the request flag; NRPA has no frontier, so we snapshot the
    /// shared best directly here.
    #[cfg(not(target_arch = "wasm32"))]
    fn do_checkpoint(&mut self) {
        if let Some(ref s) = self.search {
            match self.algo {
                SearchAlgo::Systematic | SearchAlgo::Perturbation => {
                    s.checkpoint_requested.store(true, Ordering::Relaxed)
                }
                SearchAlgo::Nrpa => nrpa::save_checkpoint(self.selected_variant, s),
                SearchAlgo::Beam => {}
            }
        }
        self.last_checkpoint = Instant::now();
    }

    #[cfg(target_arch = "wasm32")]
    fn do_checkpoint(&mut self) {}

    fn stop_search(&mut self) {
        if let Some(ref s) = self.search {
            s.running.store(false, Ordering::Relaxed);
        }
        self.search = None;
    }

    fn search_paused(&self) -> bool {
        self.search
            .as_ref()
            .map(|s| s.paused.load(Ordering::Relaxed))
            .unwrap_or(false)
    }

    /// Flip the cooperative pause on the running search. Workers idle at their
    /// next loop boundary; a later stop still unblocks them (stop beats pause).
    fn toggle_pause(&mut self) {
        if let Some(ref s) = self.search {
            let now = !s.paused.load(Ordering::Relaxed);
            s.paused.store(now, Ordering::Relaxed);
            self.status = Some(if now {
                fl!(LANGUAGE_LOADER, "status-search-paused")
            } else {
                fl!(LANGUAGE_LOADER, "status-search-resumed")
            });
        }
    }

    fn search_running(&self) -> bool {
        self.search
            .as_ref()
            .map(|s| s.running.load(Ordering::Relaxed))
            .unwrap_or(false)
    }
}

impl eframe::App for MorpionApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let mut set = |k: &str, v: String| storage.set_string(k, v);
        set("dark_mode", self.dark_mode.to_string());
        set("hide_rules", self.hide_rules.to_string());
        set("default_author", self.default_author.clone());
        // View/interaction toggles and the search configuration — restored in `new`.
        set("view_arrows", self.view_arrows.to_string());
        set("view_numbers", self.view_numbers.to_string());
        set("show_legal", self.show_legal.to_string());
        set("nrpa_level", self.nrpa_level.to_string());
        if let Ok(s) = serde_json::to_string(&self.selected_variant) {
            set("variant", s);
        }
        if let Ok(s) = serde_json::to_string(&self.input_mode) {
            set("input_mode", s);
        }
        if let Ok(s) = serde_json::to_string(&self.algo) {
            set("algo", s);
        }
        if let Ok(s) = serde_json::to_string(&self.start_point) {
            set("start_point", s);
        }
        if let Ok(s) = serde_json::to_string(&self.export_format) {
            set("export_format", s);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // eframe 0.34 drives the app through `ui` (the root viewport Ui); the
        // panels are shown *inside* it. The rest of this method works at the
        // context level (floating areas/windows, input, repaint), so take a cheap
        // (`Arc`) clone of the context to keep those calls unchanged.
        let ctx = ui.ctx().clone();
        // Keep egui's theme in step with `dark_mode`. eframe can apply the
        // platform/browser theme over the choice made in `new()`, which would
        // desync the menu (egui visuals) from the board (which follows
        // `dark_mode`); reassert it here, cheaply — only when it has drifted.
        if ctx.global_style().visuals.dark_mode != self.dark_mode {
            ctx.set_visuals(if self.dark_mode {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
        }
        // Repaint every frame while search is running so stats stay live.
        if self.search_running() {
            ctx.request_repaint();
        }

        let mut played_move: Option<Move> = None;

        // Snapshot search stats for display.
        let (nodes_explored, best_search_score) = if let Some(ref s) = self.search {
            (
                s.nodes_explored.load(Ordering::Relaxed),
                s.best_score.load(Ordering::Relaxed),
            )
        } else {
            (0, 0)
        };

        // Update nodes/sec rate every ~0.5s.
        {
            let elapsed = self.last_rate_time.elapsed().as_secs_f64();
            if elapsed >= 0.5 {
                let delta = nodes_explored.saturating_sub(self.last_rate_nodes);
                self.nodes_per_sec = delta as f64 / elapsed;
                self.last_rate_nodes = nodes_explored;
                self.last_rate_time = Instant::now();
            }
            if !self.search_running() {
                self.nodes_per_sec = 0.0;
                self.last_rate_nodes = 0;
            }
        }

        // Tick the search timer while running; it freezes once the search stops.
        if self.search_running() {
            self.search_elapsed = self.search_start.elapsed();
        }

        // The systematic search can drain the whole tree on its own (only feasible
        // for the small variants). When it does, its best score is provably optimal
        // — announce it once.
        if !self.exhausted_seen {
            if let Some(ref s) = self.search {
                if !s.running.load(Ordering::Relaxed) && s.exhausted.load(Ordering::Relaxed) {
                    self.exhausted_seen = true;
                    let best = s.best_score.load(Ordering::Relaxed);
                    self.exhausted_notice = Some((best, self.search_elapsed));
                }
            }
        }

        // Auto-checkpoint the running search at a fixed interval (systematic and
        // NRPA; both support resume).
        if self.search_running()
            && !self.search_paused()
            && matches!(
                self.algo,
                SearchAlgo::Systematic | SearchAlgo::Nrpa | SearchAlgo::Perturbation
            )
            && self.last_checkpoint.elapsed().as_secs_f64() >= AUTO_CHECKPOINT_SECS
        {
            self.do_checkpoint();
        }

        // Rebuild preview state when a better sequence is found.
        if best_search_score > self.search_preview_score {
            if let Some(ref s) = self.search {
                let seq = s.best_sequence.read().unwrap().clone();
                if !seq.is_empty() {
                    let mut preview = GameState::new(self.selected_variant);
                    for mv in &seq {
                        preview.apply(*mv);
                    }
                    self.search_preview_legal = legal_moves(&preview).len();
                    self.search_preview = Some(preview);
                    self.search_preview_score = best_search_score;
                }
            }
        }

        // Auto-save every new record. A record only counts when the position is
        // terminal (0 available moves) and beats the best this algorithm category
        // has already saved (each category has its own subdirectory & threshold).
        let category_best = *self.saved_best.get(self.record_category).unwrap_or(&0);
        if best_search_score > category_best
            && self.search_preview.is_some()
            && self.search_preview_legal == 0
        {
            self.saved_best
                .insert(self.record_category, best_search_score);
            self.persist_record(best_search_score);
        }

        // Audio alarm when the 5T world record (178) is beaten — a rare, exciting
        // event worth interrupting whatever you're doing. Fires once per search.
        if best_search_score > WORLD_RECORD_5T && !self.alarm_fired {
            self.alarm_fired = true;
            self.alarm_active.store(true, Ordering::Relaxed);
            self.status = Some(fl!(
                LANGUAGE_LOADER,
                "status-record-beaten",
                score = (best_search_score as i64),
                record = (WORLD_RECORD_5T as i64)
            ));
            #[cfg(not(target_arch = "wasm32"))]
            play_alarm_loop(self.alarm_active.clone());
        }

        // Grid overflow: a search stepped to the edge of the fixed grid. The cheap
        // check lives in `Board::insert`; here we just poll the flag, then save the
        // best, stop, and alert — never crash. `swap` consumes the flag so the
        // routine runs once even though winding-down threads may re-set it.
        if crate::game::board::GRID_OVERFLOW.swap(false, Ordering::Relaxed) && self.search_running()
        {
            self.handle_grid_overflow(best_search_score);
        }

        // Keyboard clipboard: Ctrl-C copies the shown position, Ctrl-V imports.
        let (copy, paste) = ctx.input(|i| {
            let copy = i.events.iter().any(|e| matches!(e, egui::Event::Copy));
            let paste = i.events.iter().find_map(|e| match e {
                egui::Event::Paste(t) => Some(t.clone()),
                _ => None,
            });
            (copy, paste)
        });
        if copy {
            self.copy_to_clipboard(&ctx);
        }
        if let Some(text) = paste {
            self.try_import(&text);
        }

        // Shortcuts (only when no text field is focused). Plain keys: R rotate,
        // F flip, G recenter — mirroring the board's overlaid icon buttons.
        // Command (Ctrl / ⌘) combos: Z undo, R redo, N new game, S export.
        if !ctx.egui_wants_keyboard_input() {
            let k = ctx.input(|i| {
                let cmd = i.modifiers.command;
                let plain = !cmd;
                [
                    plain && i.key_pressed(egui::Key::R), // rotate
                    plain && i.key_pressed(egui::Key::F), // flip
                    plain && i.key_pressed(egui::Key::G), // recenter
                    cmd && i.key_pressed(egui::Key::Z),   // undo
                    cmd && i.key_pressed(egui::Key::R),   // redo
                    cmd && i.key_pressed(egui::Key::N),   // new game
                    cmd && i.key_pressed(egui::Key::S),   // export
                ]
            });
            let [rot, flip, recenter, undo, redo, new_game, export] = k;
            if rot {
                self.rotate_view();
            }
            if flip {
                self.flip_view();
            }
            if recenter {
                self.recenter_view();
            }
            // Undo/redo only when the board is editable (no search preview shown).
            if self.search_preview.is_none() {
                if undo {
                    self.state.undo();
                    self.refresh_legal();
                    self.dirty = true;
                }
                if redo {
                    self.state.redo();
                    self.refresh_legal();
                    self.dirty = true;
                }
            }
            if new_game {
                self.request(PendingAction::NewGame(self.selected_variant));
            }
            if export {
                // File export is native; the web build copies instead.
                #[cfg(not(target_arch = "wasm32"))]
                self.export_to_file();
                #[cfg(target_arch = "wasm32")]
                self.copy_to_clipboard(&ctx);
            }
        }

        // Score / available-moves shown in the panel follow the *displayed*
        // position: the preview while a search result is on screen.
        let (disp_score, disp_legal) = match self.search_preview.as_ref() {
            Some(p) => (p.score(), self.search_preview_legal),
            None => (self.state.score(), self.legal.len()),
        };

        // Checkpoint/resume is native-only; the checkpoint file is only inspected
        // while idle (it touches the filesystem), short-circuiting during a search.
        let checkpoint_supported = !cfg!(target_arch = "wasm32");
        let resume = (!self.search_running())
            .then(|| resume_info(self.algo))
            .flatten();

        // Only offer records playable on the selected variant. `record_idx` maps a
        // position in the filtered dropdown back to its index in `RECORDS`.
        let (record_names, record_idx): (Vec<String>, Vec<usize>) = self
            .record_meta
            .iter()
            .enumerate()
            .filter(|(_, (v, _))| *v == self.selected_variant)
            .map(|(i, (_, label))| (label.clone(), i))
            .unzip();

        // On a narrow (phone) screen the controls become an overlay toggled by a
        // button so the board can use the full width; on wider screens the panel
        // stays docked as before. 550px ≈ below a small tablet in portrait.
        let narrow = ctx.content_rect().width() < 550.0;
        if !narrow || self.controls_open {
            let panel = egui::Panel::right("controls_panel");
            let panel = if narrow {
                // Cover the board on a phone; not user-resizable.
                panel
                    .resizable(false)
                    .exact_size(ctx.content_rect().width())
            } else {
                panel.min_size(220.0).max_size(300.0)
            };
            panel.show_inside(ui, |ui| {
                let out = controls::show(
                    ui,
                    &controls::ControlsInput {
                        variant: self.selected_variant,
                        algo: self.algo,
                        showing_preview: self.search_preview.is_some(),
                        nrpa_level: self.nrpa_level,
                        start_point: self.start_point,
                        export_format: self.export_format,
                        dark_mode: self.dark_mode,
                        warm_available: !self.state.history.is_empty(),
                        loaded_terminal: !self.state.history.is_empty() && self.legal.is_empty(),
                        record_names: record_names.clone(),
                        alarm_active: self.alarm_active.load(Ordering::Relaxed),
                        score: disp_score,
                        legal_count: disp_legal,
                        search_running: self.search_running(),
                        search_paused: self.search_paused(),
                        nodes_explored,
                        best_search_score,
                        nodes_per_sec: self.nodes_per_sec,
                        elapsed: self.search_elapsed,
                        records: self
                            .search
                            .as_ref()
                            .map(|s| s.records.read().unwrap().clone())
                            .unwrap_or_default(),
                        checkpoint_supported,
                        resume,
                    },
                );
                if let Some(v) = out.new_game {
                    self.request(PendingAction::NewGame(v));
                }
                if let Some(a) = out.set_algo {
                    self.algo = a;
                    // Keep the starting point valid for the new algorithm.
                    if !controls::start_points_for(a).contains(&self.start_point) {
                        self.start_point = StartPoint::Empty;
                    }
                }
                if let Some(level) = out.set_nrpa_level {
                    self.nrpa_level = level;
                }
                if let Some(sp) = out.set_start_point {
                    self.start_point = sp;
                }
                if let Some(j) = out.load_record {
                    if let Some((_, _, msr)) = record_idx.get(j).and_then(|&i| RECORDS.get(i)) {
                        self.request(PendingAction::LoadMsr(msr));
                    }
                }
                if out.start_search {
                    self.start_search();
                }
                if out.stop_search {
                    self.stop_search();
                }
                if out.toggle_pause {
                    self.toggle_pause();
                }
                if out.checkpoint {
                    self.do_checkpoint();
                    self.status = Some(fl!(LANGUAGE_LOADER, "status-checkpoint"));
                }
                if out.resume_search {
                    self.resume_search();
                }
                if out.load_best {
                    if let Some(preview) = self.search_preview.take() {
                        self.state = preview;
                        self.refresh_legal();
                        self.stop_search();
                        self.search_preview_score = 0;
                    }
                }
                if out.dismiss_preview {
                    self.stop_search();
                    self.search_preview = None;
                    self.search_preview_score = 0;
                }
                if let Some(f) = out.set_export_format {
                    self.export_format = f;
                }
                if out.copy {
                    self.request_export(ExportAction::Copy, ui.ctx());
                }
                if out.export_file {
                    self.request_export(ExportAction::File, ui.ctx());
                }
                if out.import {
                    self.import_open = !self.import_open;
                }
                if out.silence_alarm {
                    self.alarm_active.store(false, Ordering::Relaxed);
                }
                if out.toggle_theme {
                    self.dark_mode = !self.dark_mode;
                    ui.ctx().set_visuals(if self.dark_mode {
                        egui::Visuals::dark()
                    } else {
                        egui::Visuals::light()
                    });
                }
                if out.show_shortcuts {
                    self.shortcuts_open = true;
                }
                if out.show_rules {
                    self.rules_open = true;
                }

                // Editorial metadata editor — applied to manual exports and filled
                // in from imported records.
                {
                    let l = &*LANGUAGE_LOADER;
                    ui.separator();
                    egui::CollapsingHeader::new(fl!(l, "meta-title"))
                        .id_salt("meta_editor")
                        .show(ui, |ui| {
                            egui::Grid::new("meta_grid")
                                .num_columns(2)
                                .spacing([8.0, 6.0])
                                .show(ui, |ui| {
                                    let line =
                                        |ui: &mut egui::Ui, label: String, v: &mut String| {
                                            ui.label(label);
                                            ui.add(
                                                egui::TextEdit::singleline(v)
                                                    .desired_width(f32::INFINITY),
                                            );
                                            ui.end_row();
                                        };
                                    line(ui, fl!(l, "meta-author"), &mut self.meta.author);
                                    line(ui, fl!(l, "meta-source"), &mut self.meta.source);
                                    line(
                                        ui,
                                        fl!(l, "meta-transcribed-by"),
                                        &mut self.meta.transcribed_by,
                                    );
                                    ui.label(fl!(l, "meta-description"));
                                    ui.add(
                                        egui::TextEdit::multiline(&mut self.meta.description)
                                            .desired_rows(2)
                                            .desired_width(f32::INFINITY),
                                    );
                                    ui.end_row();
                                    ui.label(fl!(l, "meta-tags"));
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.meta.tags)
                                            .desired_width(f32::INFINITY)
                                            .hint_text(fl!(l, "meta-tags-hint")),
                                    );
                                    ui.end_row();
                                });
                        });
                }

                // Inline import box (paste a JSON or Pentasol save).
                if self.import_open {
                    let l = &*LANGUAGE_LOADER;
                    ui.separator();
                    ui.label(fl!(l, "import-hint"));
                    ui.add(
                        egui::TextEdit::multiline(&mut self.import_text)
                            .desired_rows(4)
                            .desired_width(f32::INFINITY)
                            .code_editor(),
                    );
                    ui.horizontal(|ui| {
                        if ui.button(fl!(l, "btn-load")).clicked() {
                            let text = self.import_text.clone();
                            self.try_import(&text);
                        }
                        if ui.button(fl!(l, "btn-cancel")).clicked() {
                            self.import_open = false;
                        }
                    });
                }

                if let Some(ref status) = self.status {
                    ui.add_space(6.0);
                    ui.separator();
                    ui.label(egui::RichText::new(status).italics().weak());
                }

                // Footer: version · licence · copyright, then the relevant links.
                let l = &*LANGUAGE_LOADER;
                ui.add_space(8.0);
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    // Non-breaking spaces inside the copyright so egui never wraps
                    // in the middle of the name (epaint treats U+00A0 as unbreakable).
                    ui.weak(concat!(
                        "v",
                        env!("CARGO_PKG_VERSION"),
                        " · ",
                        env!("CARGO_PKG_LICENSE"),
                        " · ©\u{a0}Stéphane\u{a0}Jourdois"
                    ));
                    ui.hyperlink_to(fl!(l, "link-docs"), "https://morpion-solitaire.io/docs/");
                    ui.hyperlink_to(
                        fl!(l, "link-source"),
                        "https://github.com/sjourdois/morpion-solitaire",
                    );
                });
            });
        }

        // Mobile: a floating button toggles the controls overlay (drawn on top).
        if narrow {
            let open = self.controls_open;
            egui::Area::new(egui::Id::new("controls_toggle"))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 8.0))
                .show(&ctx, |ui| {
                    let icon = if open { Icon::Close } else { Icon::Menu };
                    if icons::icon_button(ui, icon, open, true).clicked() {
                        self.controls_open = !open;
                    }
                });
        }

        // The board occupies whatever the side panels leave free; capture it so the
        // overlaid toolbars anchor to the board's corners rather than the screen's
        // (otherwise the right-hand toolbar lands over the controls panel). On a
        // phone with the controls overlay open the panel covers the board, so the
        // board toolbars and search overlay are suppressed to avoid drawing on top.
        let controls_cover_board = narrow && self.controls_open;
        // Whatever the side panel left free is the board region (matches the old
        // context-level `available_rect`).
        let board_rect = ui.available_rect_before_wrap();

        let dark = self.dark_mode;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            let (vr, vf, va, vn, sl) = (
                self.view_rot,
                self.view_flip,
                self.view_arrows,
                self.view_numbers,
                self.show_legal,
            );
            if let Some(ref preview) = self.search_preview {
                // A search result (live or finished) is read-only: no hover, no clicks.
                let mut no_hover = None;
                board_view::show(
                    ui,
                    preview,
                    &[],
                    &mut no_hover,
                    vr,
                    vf,
                    va,
                    vn,
                    sl,
                    self.input_mode,
                    &mut self.view_zoom,
                    &mut self.view_pan,
                    dark,
                );
            } else {
                // No preview: the played game is editable.
                played_move = board_view::show(
                    ui,
                    &self.state,
                    &self.legal,
                    &mut self.hovered,
                    vr,
                    vf,
                    va,
                    vn,
                    sl,
                    self.input_mode,
                    &mut self.view_zoom,
                    &mut self.view_pan,
                    dark,
                );
            }
        });

        if !controls_cover_board {
            self.board_toolbars(&ctx, board_rect);
            self.search_overlay(&ctx, board_rect);
        }
        self.shortcuts_window(&ctx);
        self.rules_window(&ctx);

        if let Some(mv) = played_move {
            self.state.apply(mv);
            self.refresh_legal();
            self.dirty = true;
        }

        // Unsaved-changes confirmation for a deferred board-replacing action.
        self.confirm_pending(&ctx);
        self.author_prompt_window(&ctx);
        self.exhausted_window(&ctx);
        self.handle_dropped_files(&ctx);
    }
}

/// Records directory under the XDG data dir:
/// `$XDG_DATA_HOME/morpion-solitaire/records` (falling back to
/// `~/.local/share` per the XDG Base Directory spec, then `.`).
/// Record categories, each with its own subdirectory and threshold. Used only by
/// the native record-saving paths (the web build has no filesystem).
#[cfg(not(target_arch = "wasm32"))]
const RECORD_CATEGORIES: [&str; 4] = ["systematic", "nrpa", "nrpa-seeded", "beam"];

#[cfg(not(target_arch = "wasm32"))]
fn records_dir(category: &str) -> std::path::PathBuf {
    use std::path::PathBuf;
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("morpion-solitaire")
        .join("records")
        .join(category)
}

/// Loop an audio alarm on a background thread until `active` is cleared (the
/// Silence button). Each beep shells out to whichever desktop sound player is
/// present (PipeWire / PulseAudio / ALSA) with a system sound; no build-time
/// audio dependency. Falls back to the terminal bell. Never blocks the UI.
#[cfg(not(target_arch = "wasm32"))]
fn play_alarm_loop(active: Arc<std::sync::atomic::AtomicBool>) {
    std::thread::spawn(move || {
        use std::process::{Command, Stdio};
        use std::time::Duration;
        const OGA: &str = "/usr/share/sounds/freedesktop/stereo/complete.oga";
        const WAV: &str = "/usr/share/sounds/alsa/Front_Center.wav";
        let attempts: [(&str, &str); 3] = [("pw-play", OGA), ("paplay", OGA), ("aplay", WAV)];
        while active.load(Ordering::Relaxed) {
            let played = attempts.iter().any(|(cmd, sound)| {
                Command::new(cmd)
                    .arg(sound)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            });
            if !played {
                eprint!("\x07");
            }
            // Pause between beeps, staying responsive to silencing.
            for _ in 0..6 {
                if !active.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    });
}

/// Checkpoint-file tag for an algorithm (its checkpoints live in a per-algo
/// file). Beam has no checkpoint support. Native only — checkpointing is a
/// no-op on the web.
#[cfg(not(target_arch = "wasm32"))]
fn algo_tag(algo: SearchAlgo) -> &'static str {
    match algo {
        SearchAlgo::Systematic => "systematic",
        SearchAlgo::Nrpa => "nrpa",
        SearchAlgo::Beam => "beam",
        SearchAlgo::Perturbation => "perturbation",
    }
}

/// Metadata for the Resume button when a saved checkpoint exists for `algo`: the
/// algo plus how long ago it was written (from the file mtime — cheap, no
/// deserialise of a possibly-huge frontier). `None` means nothing to resume.
#[cfg(not(target_arch = "wasm32"))]
fn resume_info(algo: SearchAlgo) -> Option<ResumeInfo> {
    let path = crate::search::checkpoint::path(algo_tag(algo));
    let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok()?;
    let age = modified.elapsed().unwrap_or_default();
    Some(ResumeInfo { algo, age })
}

#[cfg(target_arch = "wasm32")]
fn resume_info(_algo: SearchAlgo) -> Option<ResumeInfo> {
    None
}

/// Write a record-breaking position to the records directory.
/// Filename: `<score:03>.msr` — the zero-padded score makes the files sort by
/// record length. Content is the compact `MS1:` blob (the timestamp lives
/// inside it). The `epoch` argument is kept for signature symmetry.
#[cfg(not(target_arch = "wasm32"))]
fn save_record_file(blob: &str, score: u32, category: &str) -> std::io::Result<std::path::PathBuf> {
    let dir = records_dir(category);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{score:03}.msr"));
    // Trailing newline so `cat` of the file doesn't end mid-line.
    std::fs::write(&path, format!("{blob}\n"))?;
    Ok(path)
}

/// Scan the records directory and return the highest record score saved so far
/// (0 if none / unreadable). Handles the current `<score>.msr` names as well as
/// the legacy `morpion-<score>coups-<epoch>.{msr,json}` names.
#[cfg(not(target_arch = "wasm32"))]
fn highest_saved_record(category: &str) -> u32 {
    let Ok(entries) = std::fs::read_dir(records_dir(category)) else {
        return 0;
    };
    let mut best = 0u32;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let stem = name
            .strip_suffix(".msr")
            .or_else(|| name.strip_suffix(".json"))
            .unwrap_or(&name);
        // Current format: the stem is the (zero-padded) score itself.
        let score = stem.parse::<u32>().ok().or_else(|| {
            // Legacy: morpion-<score>coups-<epoch>
            stem.strip_prefix("morpion-")
                .and_then(|r| r.split("coups").next())
                .and_then(|n| n.parse::<u32>().ok())
        });
        if let Some(score) = score {
            best = best.max(score);
        }
    }
    best
}

/// Highest saved record per category, scanned from disk at launch.
fn load_saved_bests() -> std::collections::HashMap<&'static str, u32> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        RECORD_CATEGORIES
            .iter()
            .map(|&c| (c, highest_saved_record(c)))
            .collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        std::collections::HashMap::new() // no filesystem on the web
    }
}

/// Trigger a browser download of `bytes` as `filename`: build a `Blob`, point a
/// throwaway `<a download>` at it, click it, then revoke the object URL. This is
/// the web build's stand-in for the native save dialog.
#[cfg(target_arch = "wasm32")]
fn browser_download(filename: &str, bytes: &[u8]) -> Result<(), String> {
    use wasm_bindgen::JsCast;
    let doc = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("no document")?;
    let body = doc.body().ok_or("no document body")?;

    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type("application/octet-stream");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts)
        .map_err(|_| "could not build the file blob".to_owned())?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "object URL failed".to_owned())?;

    let anchor = doc
        .create_element("a")
        .and_then(|e| {
            e.dyn_into::<web_sys::HtmlAnchorElement>()
                .map_err(Into::into)
        })
        .map_err(|_| "could not build the download link".to_owned())?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    let _ = body.append_child(&anchor);
    anchor.click();
    let _ = body.remove_child(&anchor);
    web_sys::Url::revoke_object_url(&url).ok();
    Ok(())
}
