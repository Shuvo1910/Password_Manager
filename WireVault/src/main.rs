#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use eframe::egui::{
    self, pos2, vec2, Align, Align2, Color32, FontId, Margin, Pos2, Rect, RichText, Sense, Stroke,
};
use password_manager::{
    crypto,
    vault::{Entry, UnlockedVault},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, Instant},
};
use zeroize::{Zeroize, Zeroizing};

// ───────────────────────────── palette ─────────────────────────────

mod pal {
    use eframe::egui::Color32 as C;
    pub const BG: C = C::from_rgb(28, 28, 30);
    pub const SIDEBAR: C = C::from_rgb(38, 38, 41);
    pub const LIST: C = C::from_rgb(32, 32, 34);
    pub const CARD: C = C::from_rgb(42, 42, 45);
    pub const FIELD: C = C::from_rgb(56, 56, 60);
    pub const SEP: C = C::from_rgb(64, 64, 69);
    pub const TEXT: C = C::from_rgb(245, 245, 247);
    pub const TEXT2: C = C::from_rgb(160, 160, 168);
    pub const TEXT3: C = C::from_rgb(112, 112, 120);
    pub const ACCENT: C = C::from_rgb(10, 132, 255);
    pub const GREEN: C = C::from_rgb(50, 215, 75);
    pub const RED: C = C::from_rgb(255, 69, 58);
    pub const ORANGE: C = C::from_rgb(255, 159, 10);
    pub const YELLOW: C = C::from_rgb(255, 204, 0);
}

const AUTO_LOCK: Duration = Duration::from_secs(300);
const CLIPBOARD_CLEAR: Duration = Duration::from_secs(30);
const MIN_MASTER_LEN: usize = 12;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Password Manager")
            // Standard medium window (3:2 landscape); users can still maximize.
            .with_inner_size([1080.0, 720.0])
            .with_min_inner_size([920.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Password Manager",
        options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            setup_theme(&cc.egui_ctx);
            Ok(Box::new(PasswordManagerApp::default()))
        }),
    )
}

/// Use the native Windows UI font (Segoe UI) when it is installed. This is a
/// local read of a system font; nothing is downloaded or written.
fn setup_fonts(ctx: &egui::Context) {
    let Some(windir) = std::env::var_os("WINDIR") else {
        return;
    };
    let path = PathBuf::from(windir).join("Fonts").join("segoeui.ttf");
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "segoe-ui".to_owned(),
        std::sync::Arc::new(egui::FontData::from_owned(bytes)),
    );
    if let Some(family) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        family.insert(0, "segoe-ui".to_owned());
    }
    ctx.set_fonts(fonts);
}

fn setup_theme(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.dark_mode = true;
    v.panel_fill = pal::BG;
    v.window_fill = pal::CARD;
    v.faint_bg_color = pal::CARD;
    v.extreme_bg_color = pal::FIELD;
    v.code_bg_color = pal::FIELD;
    v.window_stroke = Stroke::new(1.0_f32, pal::SEP);
    v.window_corner_radius = egui::CornerRadius::same(14);
    v.menu_corner_radius = egui::CornerRadius::same(10);
    v.hyperlink_color = pal::ACCENT;
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(10, 132, 255, 110);
    v.selection.stroke = Stroke::new(1.5_f32, pal::ACCENT);

    let radius = egui::CornerRadius::same(8);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = pal::CARD;
    w.noninteractive.weak_bg_fill = pal::CARD;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, pal::SEP);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, pal::TEXT);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = pal::FIELD;
    w.inactive.weak_bg_fill = Color32::from_rgb(64, 64, 69);
    w.inactive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(78, 78, 84));
    w.inactive.fg_stroke = Stroke::new(1.0_f32, pal::TEXT);
    w.inactive.corner_radius = radius;

    w.hovered.bg_fill = Color32::from_rgb(70, 70, 76);
    w.hovered.weak_bg_fill = Color32::from_rgb(76, 76, 82);
    w.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(98, 98, 106));
    w.hovered.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    w.hovered.corner_radius = radius;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = Color32::from_rgb(84, 84, 91);
    w.active.weak_bg_fill = Color32::from_rgb(90, 90, 97);
    w.active.bg_stroke = Stroke::new(1.0_f32, pal::ACCENT);
    w.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    w.active.corner_radius = radius;
    w.active.expansion = 0.0;

    w.open.bg_fill = Color32::from_rgb(70, 70, 76);
    w.open.weak_bg_fill = Color32::from_rgb(70, 70, 76);
    w.open.corner_radius = radius;

    let mut style = (*ctx.style()).clone();
    style.visuals = v;
    style.spacing.item_spacing = vec2(10.0, 8.0);
    style.spacing.button_padding = vec2(14.0, 7.0);
    style.spacing.interact_size.y = 32.0;
    style.spacing.scroll = egui::style::ScrollStyle::floating();
    style
        .text_styles
        .insert(egui::TextStyle::Heading, FontId::proportional(22.0));
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(12.0));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, FontId::monospace(14.0));
    ctx.set_style(style);
}

// ───────────────────────────── app state ─────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Welcome,
    Create,
    RecoveryConfirm,
    Unlock,
    Dashboard,
}

#[derive(Clone, Copy, PartialEq)]
enum Section {
    All,
    Favorites,
    Recent,
    Settings,
}

#[derive(Clone, Copy, PartialEq)]
enum Icon {
    Lock,
    Star,
    Clock,
    Gear,
}

#[derive(Default, Clone, PartialEq)]
struct Status {
    text: String,
    ok: bool,
}
impl Status {
    fn ok(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ok: true,
        }
    }
    fn err(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ok: false,
        }
    }
    fn clear(&mut self) {
        self.text.clear();
    }
}

#[derive(Default, Clone)]
struct EntryForm {
    title: String,
    username: String,
    password: String,
    url: String,
    notes: String,
    favorite: bool,
    show: bool,
    icon_mime: Option<String>,
    icon_bytes: Option<Vec<u8>>,
}

struct Row {
    id: uuid::Uuid,
    title: String,
    username: String,
    favorite: bool,
    created: u64,
    has_icon: bool,
}

enum DetailAction {
    CopyUser,
    CopyPass,
    CopyUrl,
    ToggleReveal,
    ToggleFavorite,
    Edit,
    Delete,
}

struct PasswordManagerApp {
    screen: Screen,
    section: Section,
    vault: Option<UnlockedVault>,
    vault_path: Option<PathBuf>,
    status: Status,
    status_seen: Status,
    status_since: Instant,
    master_password: String,
    confirm_password: String,
    current_password: String,
    new_master_password: String,
    show_recovery: bool,
    show_recover_form: bool,
    recovery_key: String,
    recovery_confirmation: String,
    pending_recovery: String,
    rotating_recovery: bool,
    confirm_rotate: bool,
    show_password: bool,
    reveal: bool,
    search: String,
    selected: Option<uuid::Uuid>,
    adding: bool,
    edit_id: Option<uuid::Uuid>,
    confirm_delete: Option<uuid::Uuid>,
    form: EntryForm,
    form_icon_tex: Option<egui::TextureHandle>,
    icons: HashMap<uuid::Uuid, Option<egui::TextureHandle>>,
    generator_length: usize,
    generator_upper: bool,
    generator_lower: bool,
    generator_digits: bool,
    generator_symbols: bool,
    generated: String,
    clip_hash: Option<[u8; 32]>,
    clip_deadline: Instant,
    last_activity: Instant,
    startup_size_adjusted: bool,
}

impl Default for PasswordManagerApp {
    fn default() -> Self {
        Self {
            screen: Screen::Welcome,
            section: Section::All,
            vault: None,
            vault_path: None,
            status: Status::default(),
            status_seen: Status::default(),
            status_since: Instant::now(),
            master_password: String::new(),
            confirm_password: String::new(),
            current_password: String::new(),
            new_master_password: String::new(),
            show_recovery: false,
            show_recover_form: false,
            recovery_key: String::new(),
            recovery_confirmation: String::new(),
            pending_recovery: String::new(),
            rotating_recovery: false,
            confirm_rotate: false,
            show_password: false,
            reveal: false,
            search: String::new(),
            selected: None,
            adding: false,
            edit_id: None,
            confirm_delete: None,
            form: EntryForm::default(),
            form_icon_tex: None,
            icons: HashMap::new(),
            generator_length: 20,
            generator_upper: true,
            generator_lower: true,
            generator_digits: true,
            generator_symbols: true,
            generated: String::new(),
            clip_hash: None,
            clip_deadline: Instant::now(),
            last_activity: Instant::now(),
            startup_size_adjusted: false,
        }
    }
}

// ───────────────────────────── app logic ─────────────────────────────

impl PasswordManagerApp {
    fn open_create(&mut self) {
        self.master_password.zeroize();
        self.confirm_password.zeroize();
        self.status.clear();
        self.screen = Screen::Create;
    }

    fn choose_new_vault(&mut self) {
        // Validate first so the user is not sent through a file dialog only to
        // be told the passwords were wrong.
        if self.master_password != self.confirm_password {
            self.status = Status::err("Master passwords do not match.");
            return;
        }
        if self.master_password.len() < MIN_MASTER_LEN {
            self.status = Status::err("Use a master password of at least 12 characters.");
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save Password Manager vault")
            .add_filter("Password Manager vault", &["vault"])
            .save_file()
        else {
            return;
        };
        if path.exists() {
            self.status = Status::err(
                "That file already exists. Choose a new filename; Password Manager will not overwrite an existing vault.",
            );
            return;
        }
        match UnlockedVault::create(self.master_password.as_bytes()) {
            Ok((vault, recovery)) => {
                self.vault = Some(vault);
                self.vault_path = Some(path);
                self.pending_recovery = recovery;
                self.recovery_confirmation.clear();
                self.master_password.zeroize();
                self.confirm_password.zeroize();
                self.rotating_recovery = false;
                self.status.clear();
                self.screen = Screen::RecoveryConfirm;
            }
            Err(e) => self.status = Status::err(e.to_string()),
        }
    }

    fn finish_creation(&mut self) {
        let matches = crypto::decode_recovery_key(&self.recovery_confirmation)
            .ok()
            .zip(crypto::decode_recovery_key(&self.pending_recovery).ok())
            .is_some_and(|(a, b)| a == b);
        if !matches {
            self.status =
                Status::err("The recovery key does not match. Re-enter it exactly as shown.");
            return;
        }
        if let (Some(vault), Some(path)) = (self.vault.as_mut(), self.vault_path.clone()) {
            match vault.save_as(&path) {
                Ok(()) => {
                    self.pending_recovery.zeroize();
                    self.recovery_confirmation.zeroize();
                    self.screen = Screen::Dashboard;
                    self.section = Section::All;
                    self.last_activity = Instant::now();
                    self.status = if self.rotating_recovery {
                        Status::ok("Recovery key replaced. The previous key no longer works.")
                    } else {
                        Status::ok("Vault created and saved.")
                    };
                    self.rotating_recovery = false;
                }
                Err(e) => self.status = Status::err(e.to_string()),
            }
        }
    }

    fn open_existing(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Open existing Password Manager vault")
            .add_filter("Password Manager vault", &["vault"])
            .pick_file()
        else {
            return;
        };
        self.vault_path = Some(path);
        self.master_password.zeroize();
        self.recovery_key.zeroize();
        self.new_master_password.zeroize();
        self.confirm_password.zeroize();
        self.show_recover_form = false;
        self.status.clear();
        self.screen = Screen::Unlock;
    }

    fn change_master(&mut self) {
        let Some(vault) = self.vault.as_mut() else {
            return;
        };
        let Some(path) = vault.path().map(PathBuf::from) else {
            self.status = Status::err("Vault path is unavailable.");
            return;
        };
        match UnlockedVault::unlock_password(&path, self.current_password.as_bytes()) {
            Ok(_)
                if self.new_master_password.len() >= MIN_MASTER_LEN
                    && self.new_master_password == self.confirm_password =>
            {
                match vault.change_master_password(self.new_master_password.as_bytes()) {
                    Ok(()) => {
                        self.status = Status::ok("Master password changed.");
                        self.current_password.zeroize();
                        self.new_master_password.zeroize();
                        self.confirm_password.zeroize();
                    }
                    Err(e) => self.status = Status::err(e.to_string()),
                }
            }
            Ok(_) => {
                self.status =
                    Status::err("Enter matching new master passwords (at least 12 characters).")
            }
            Err(_) => self.status = Status::err("Current master password is incorrect."),
        }
    }

    fn unlock(&mut self) {
        let Some(path) = self.vault_path.clone() else {
            self.status = Status::err("Choose a vault file first.");
            return;
        };
        match UnlockedVault::unlock_password(&path, self.master_password.as_bytes()) {
            Ok(v) => {
                self.vault = Some(v);
                self.master_password.zeroize();
                self.screen = Screen::Dashboard;
                self.section = Section::All;
                self.status.clear();
                self.last_activity = Instant::now();
            }
            Err(_) => {
                self.master_password.zeroize();
                self.status =
                    Status::err("Could not unlock vault. Check the password or use recovery.")
            }
        }
    }

    fn recover(&mut self) {
        let Some(path) = self.vault_path.clone() else {
            return;
        };
        match UnlockedVault::unlock_recovery(&path, &self.recovery_key) {
            Ok(mut v) => {
                if self.new_master_password.len() < MIN_MASTER_LEN
                    || self.new_master_password != self.confirm_password
                {
                    self.status = Status::err(
                        "Enter matching new master passwords (at least 12 characters).",
                    );
                    return;
                }
                match v.change_master_password(self.new_master_password.as_bytes()) {
                    Ok(()) => {
                        self.vault = Some(v);
                        self.new_master_password.zeroize();
                        self.confirm_password.zeroize();
                        self.recovery_key.zeroize();
                        self.show_recover_form = false;
                        self.screen = Screen::Dashboard;
                        self.section = Section::All;
                        self.last_activity = Instant::now();
                        self.status = Status::ok(
                            "Master password changed. The old password no longer unlocks this vault.",
                        );
                    }
                    Err(e) => self.status = Status::err(e.to_string()),
                }
            }
            Err(_) => {
                self.status = Status::err(
                    "Recovery failed. Check the recovery key; the vault remains locked.",
                )
            }
        }
    }

    fn lock(&mut self) {
        self.clear_clipboard_if_ours();
        self.vault = None;
        self.selected = None;
        self.reveal = false;
        self.adding = false;
        self.edit_id = None;
        self.confirm_delete = None;
        self.confirm_rotate = false;
        self.rotating_recovery = false;
        self.show_recover_form = false;
        self.section = Section::All;
        self.icons.clear();
        self.form_icon_tex = None;
        self.search.zeroize();
        self.master_password.zeroize();
        self.confirm_password.zeroize();
        self.current_password.zeroize();
        self.new_master_password.zeroize();
        self.recovery_key.zeroize();
        self.recovery_confirmation.zeroize();
        self.pending_recovery.zeroize();
        self.generated.zeroize();
        self.form.password.zeroize();
        self.form = EntryForm::default();
        self.screen = Screen::Welcome;
        self.status = Status::ok("Vault locked. Re-open the vault to continue.");
    }

    fn new_entry(&mut self) {
        self.form.password.zeroize();
        self.form = EntryForm::default();
        self.form_icon_tex = None;
        self.edit_id = None;
        self.status.clear();
        self.adding = true;
    }

    fn cancel_form(&mut self) {
        self.form.password.zeroize();
        self.form = EntryForm::default();
        self.form_icon_tex = None;
        self.edit_id = None;
        self.adding = false;
        self.status.clear();
    }

    fn save_entry(&mut self) {
        if self.form.title.trim().is_empty() || self.form.password.is_empty() {
            self.status = Status::err("Title and password are required.");
            return;
        }
        let Some(vault) = self.vault.as_mut() else {
            return;
        };
        let mut new_id = None;
        if let Some(id) = self.edit_id {
            if let Some(entry) = vault.data.entries.iter_mut().find(|e| e.id == id) {
                entry.title = self.form.title.clone();
                entry.username = self.form.username.clone();
                entry.password = self.form.password.clone();
                entry.url = self.form.url.clone();
                entry.notes = self.form.notes.clone();
                entry.favorite = self.form.favorite;
                entry.icon_mime = self.form.icon_mime.clone();
                entry.icon_bytes = self.form.icon_bytes.clone();
                entry.modified_unix = now();
                self.icons.remove(&id);
            } else {
                self.status = Status::err("The selected entry no longer exists.");
                return;
            }
        } else {
            let mut entry = Entry::new(
                self.form.title.clone(),
                self.form.username.clone(),
                self.form.password.clone(),
            );
            entry.url = self.form.url.clone();
            entry.notes = self.form.notes.clone();
            entry.favorite = self.form.favorite;
            entry.icon_mime = self.form.icon_mime.clone();
            entry.icon_bytes = self.form.icon_bytes.clone();
            new_id = Some(entry.id);
            vault.data.entries.push(entry);
        }
        match vault.save() {
            Ok(()) => {
                if let Some(id) = new_id {
                    self.selected = Some(id);
                    self.reveal = false;
                }
                self.status = Status::ok("Saved to encrypted vault.");
                self.adding = false;
                self.edit_id = None;
                self.form.password.zeroize();
                self.form = EntryForm::default();
                self.form_icon_tex = None;
            }
            Err(e) => self.status = Status::err(e.to_string()),
        }
    }

    fn choose_icon(&mut self, ctx: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Choose entry icon")
            .add_filter("Raster image", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
        else {
            return;
        };
        let result = (|| -> Result<(String, Vec<u8>), String> {
            let raw = std::fs::read(&path)
                .map_err(|_| "Could not read the selected image.".to_owned())?;
            if raw.len() > password_manager::MAX_ICON_BYTES {
                return Err("Icon file exceeds the 2 MiB limit.".into());
            }
            let mut limits = image::Limits::default();
            // Accept normal high-resolution JPG logos/photos, while keeping
            // decoded memory bounded before reducing them to icon size.
            limits.max_image_width = Some(4096);
            limits.max_image_height = Some(4096);
            limits.max_alloc = Some(64 * 1024 * 1024);
            let mut reader = image::ImageReader::new(std::io::Cursor::new(raw))
                .with_guessed_format()
                .map_err(|_| "Unsupported or malformed image.".to_owned())?;
            reader.limits(limits);
            let image = reader
                .decode()
                .map_err(|_| "Unsupported, malformed, or oversized image.".to_owned())?;
            // Keep photo-based JPG icons compact when stored in the vault.
            let image = image.thumbnail(256, 256);
            let mut png = std::io::Cursor::new(Vec::new());
            image
                .write_to(&mut png, image::ImageFormat::Png)
                .map_err(|_| "Could not normalize the image.".to_owned())?;
            let bytes = png.into_inner();
            if bytes.len() > password_manager::MAX_ICON_BYTES {
                return Err("Normalized icon exceeds the 2 MiB limit.".into());
            }
            Ok(("image/png".into(), bytes))
        })();
        match result {
            Ok((mime, bytes)) => {
                if let Some(old) = self.form.icon_bytes.as_mut() {
                    old.zeroize();
                }
                self.form_icon_tex = load_icon_texture(ctx, "form-icon".into(), &bytes);
                self.form.icon_mime = Some(mime);
                self.form.icon_bytes = Some(bytes);
                self.status = Status::ok("Icon loaded. It will be encrypted with the vault.");
            }
            Err(error) => self.status = Status::err(error),
        }
    }

    fn export_backup(&mut self) {
        let Some(vault) = self.vault.as_ref() else {
            return;
        };
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export encrypted Password Manager backup")
            .add_filter("Password Manager vault", &["vault"])
            .save_file()
        else {
            return;
        };
        if vault.path().is_some_and(|active| active == path) {
            self.status = Status::err("Choose a different destination from the active vault.");
            return;
        }
        match vault.create_encrypted_copy(&path) {
            Ok(()) => {
                self.status = Status::ok(
                    "Encrypted backup exported. It uses the same master password and recovery key.",
                )
            }
            Err(e) => self.status = Status::err(e.to_string()),
        }
    }

    fn replace_recovery_key(&mut self) {
        self.confirm_rotate = false;
        if let Some(vault) = self.vault.as_mut() {
            match vault.rotate_recovery_key() {
                Ok(key) => {
                    self.pending_recovery = key;
                    self.recovery_confirmation.clear();
                    self.rotating_recovery = true;
                    self.screen = Screen::RecoveryConfirm;
                    self.status = Status::ok("Save and verify the replacement recovery key.");
                }
                Err(e) => self.status = Status::err(e.to_string()),
            }
        }
    }

    fn edit_selected(&mut self, ctx: &egui::Context) {
        if let (Some(v), Some(id)) = (self.vault.as_ref(), self.selected) {
            if let Some(e) = v.data.entries.iter().find(|e| e.id == id) {
                self.form = EntryForm {
                    title: e.title.clone(),
                    username: e.username.clone(),
                    password: e.password.clone(),
                    url: e.url.clone(),
                    notes: e.notes.clone(),
                    favorite: e.favorite,
                    show: false,
                    icon_mime: e.icon_mime.clone(),
                    icon_bytes: e.icon_bytes.clone(),
                };
                self.form_icon_tex = e
                    .icon_bytes
                    .as_ref()
                    .and_then(|b| load_icon_texture(ctx, "form-icon".into(), b));
                self.edit_id = Some(id);
                self.status.clear();
                self.adding = true;
            }
        }
    }

    fn toggle_favorite(&mut self, id: uuid::Uuid) {
        if let Some(v) = self.vault.as_mut() {
            if let Some(entry) = v.data.entries.iter_mut().find(|x| x.id == id) {
                entry.favorite = !entry.favorite;
            }
            if let Err(err) = v.save() {
                self.status = Status::err(err.to_string());
            }
        }
    }

    fn delete_entry(&mut self, id: uuid::Uuid) {
        if let Some(v) = self.vault.as_mut() {
            v.data.entries.retain(|x| x.id != id);
            match v.save() {
                Ok(()) => {
                    self.status = Status::ok("Password deleted.");
                    self.selected = None;
                    self.reveal = false;
                    self.icons.remove(&id);
                }
                Err(err) => self.status = Status::err(err.to_string()),
            }
        }
    }

    // ── clipboard: copy only on explicit action; auto-clear only if unchanged ──

    fn copy_to_clipboard(&mut self, value: &str) {
        match arboard::Clipboard::new().and_then(|mut c| c.set_text(value.to_owned())) {
            Ok(()) => {
                self.clip_hash = Some(*blake3::hash(value.as_bytes()).as_bytes());
                self.clip_deadline = Instant::now() + CLIPBOARD_CLEAR;
                self.status = Status::ok(
                    "Copied. It will be cleared in 30 seconds if the clipboard is unchanged.",
                );
            }
            Err(_) => self.status = Status::err("Clipboard access failed."),
        }
    }

    fn clear_clipboard_if_ours(&mut self) {
        let Some(expected) = self.clip_hash.take() else {
            return;
        };
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            if let Ok(current) = clipboard.get_text() {
                if *blake3::hash(current.as_bytes()).as_bytes() == expected {
                    let _ = clipboard.clear();
                }
            }
        }
    }

    fn tick_clipboard(&mut self) {
        if self.clip_hash.is_some() && Instant::now() >= self.clip_deadline {
            self.clear_clipboard_if_ours();
        }
    }

    fn generate_password(&mut self) {
        let mut chars = Zeroizing::new(Vec::new());
        if self.generator_lower {
            chars.extend(b"abcdefghijkmnopqrstuvwxyz".iter().copied());
        }
        if self.generator_upper {
            chars.extend(b"ABCDEFGHJKLMNPQRSTUVWXYZ".iter().copied());
        }
        if self.generator_digits {
            chars.extend(b"23456789".iter().copied());
        }
        if self.generator_symbols {
            chars.extend(b"!@#$%^&*()-_=+[]{}:,.?".iter().copied());
        }
        if chars.is_empty() {
            self.status = Status::err("Select at least one character group.");
            return;
        }
        let mut out = String::new();
        // Rejection sampling avoids modulo bias when the alphabet length does
        // not divide the byte range evenly.
        let limit = 256 - (256 % chars.len());
        while out.len() < self.generator_length {
            match crypto::random_bytes::<1>() {
                Ok(r) if (r[0] as usize) < limit => {
                    out.push(chars[(r[0] as usize) % chars.len()] as char)
                }
                Ok(_) => continue,
                Err(e) => {
                    out.zeroize();
                    self.status = Status::err(e.to_string());
                    return;
                }
            }
        }
        self.generated.zeroize();
        self.generated = out;
    }

    fn ensure_icon(&mut self, ctx: &egui::Context, id: uuid::Uuid) {
        if self.icons.contains_key(&id) {
            return;
        }
        let tex = self
            .vault
            .as_ref()
            .and_then(|v| v.data.entries.iter().find(|e| e.id == id))
            .and_then(|e| e.icon_bytes.as_ref())
            .and_then(|b| load_icon_texture(ctx, format!("entry-icon-{id}"), b));
        self.icons.insert(id, tex);
    }

    fn rows(&self) -> Vec<Row> {
        let query = self.search.to_lowercase();
        let mut rows: Vec<Row> = match self.vault.as_ref() {
            Some(v) => v
                .data
                .entries
                .iter()
                .filter(|e| self.section != Section::Favorites || e.favorite)
                .filter(|e| {
                    query.is_empty()
                        || e.title.to_lowercase().contains(&query)
                        || e.username.to_lowercase().contains(&query)
                        || e.url.to_lowercase().contains(&query)
                        || e.notes.to_lowercase().contains(&query)
                })
                .map(|e| Row {
                    id: e.id,
                    title: e.title.clone(),
                    username: e.username.clone(),
                    favorite: e.favorite,
                    created: e.created_unix,
                    has_icon: e.icon_bytes.is_some(),
                })
                .collect(),
            None => Vec::new(),
        };
        if self.section == Section::Recent {
            rows.sort_by(|a, b| b.created.cmp(&a.created));
            rows.truncate(25);
        } else {
            rows.sort_by_key(|r| r.title.to_lowercase());
        }
        rows
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn load_icon_texture(
    ctx: &egui::Context,
    name: String,
    bytes: &[u8],
) -> Option<egui::TextureHandle> {
    let image = image::load_from_memory(bytes).ok()?;
    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    Some(ctx.load_texture(name, color, egui::TextureOptions::LINEAR))
}

/// Format a Unix timestamp as `YYYY-MM-DD HH:MM UTC` without extra dependencies.
fn fmt_time(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    if m <= 2 {
        y += 1;
    }
    format!(
        "{y}-{m:02}-{d:02} {:02}:{:02} UTC",
        secs / 3600,
        (secs % 3600) / 60
    )
}

// ───────────────────────────── drawing helpers ─────────────────────────────

fn paint_lock(p: &egui::Painter, c: Pos2, s: f32, col: Color32, hole: Color32) {
    let body = Rect::from_center_size(pos2(c.x, c.y + s * 0.13), vec2(s * 0.60, s * 0.44));
    let r = s * 0.17;
    let ay = body.top() - s * 0.08;
    let mut pts = vec![pos2(c.x - r, body.top() + 1.0), pos2(c.x - r, ay)];
    for i in 0..=16 {
        let a = std::f32::consts::PI + std::f32::consts::PI * (i as f32) / 16.0;
        pts.push(pos2(c.x + r * a.cos(), ay + r * a.sin()));
    }
    pts.push(pos2(c.x + r, body.top() + 1.0));
    p.add(egui::Shape::line(pts, Stroke::new(s * 0.09, col)));
    p.rect_filled(body, s * 0.09, col);
    p.circle_filled(pos2(c.x, body.center().y - s * 0.02), s * 0.055, hole);
    p.rect_filled(
        Rect::from_center_size(
            pos2(c.x, body.center().y + s * 0.06),
            vec2(s * 0.05, s * 0.12),
        ),
        1.0,
        hole,
    );
}

fn paint_star(p: &egui::Painter, c: Pos2, r: f32, fill: Option<Color32>, stroke: Stroke) {
    let pts: Vec<Pos2> = (0..10)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
            let rr = if i % 2 == 0 { r } else { r * 0.42 };
            pos2(c.x + rr * a.cos(), c.y + rr * a.sin())
        })
        .collect();
    if let Some(f) = fill {
        let mut mesh = egui::Mesh::default();
        mesh.colored_vertex(c, f);
        for q in &pts {
            mesh.colored_vertex(*q, f);
        }
        for i in 0..10u32 {
            mesh.add_triangle(0, 1 + i, 1 + (i + 1) % 10);
        }
        p.add(egui::Shape::mesh(mesh));
    }
    if stroke.width > 0.0 {
        p.add(egui::Shape::closed_line(pts, stroke));
    }
}

fn paint_icon(p: &egui::Painter, c: Pos2, s: f32, icon: Icon, col: Color32, hole: Color32) {
    let stroke = Stroke::new(1.6_f32, col);
    match icon {
        Icon::Lock => paint_lock(p, c, s * 0.82, col, hole),
        Icon::Star => paint_star(p, c, s * 0.34, Some(col), Stroke::NONE),
        Icon::Clock => {
            let r = s * 0.30;
            p.circle_stroke(c, r, stroke);
            p.line_segment([c, c + vec2(0.0, -r * 0.62)], stroke);
            p.line_segment([c, c + vec2(r * 0.48, 0.0)], stroke);
        }
        Icon::Gear => {
            let r = s * 0.30;
            p.circle_stroke(c, r * 0.62, Stroke::new(2.0_f32, col));
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::PI / 4.0;
                let d = vec2(a.cos(), a.sin());
                p.line_segment(
                    [c + d * (r * 0.85), c + d * (r * 1.2)],
                    Stroke::new(2.4_f32, col),
                );
            }
        }
    }
}

fn paint_search_icon(p: &egui::Painter, c: Pos2, r: f32, col: Color32) {
    let stroke = Stroke::new(1.6_f32, col);
    p.circle_stroke(c + vec2(-r * 0.15, -r * 0.15), r * 0.7, stroke);
    p.line_segment(
        [c + vec2(r * 0.35, r * 0.35), c + vec2(r * 0.95, r * 0.95)],
        stroke,
    );
}

fn paint_avatar(p: &egui::Painter, rect: Rect, title: &str, tex: Option<&egui::TextureHandle>) {
    let radius = rect.width() * 0.22;
    if let Some(t) = tex {
        p.rect_filled(rect, radius, Color32::from_rgb(70, 70, 76));
        let sz = t.size_vec2();
        let scale = (rect.width() / sz.x).min(rect.height() / sz.y) * 0.86;
        let img = Rect::from_center_size(rect.center(), sz * scale);
        p.image(
            t.id(),
            img,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        return;
    }
    const COLORS: [Color32; 8] = [
        Color32::from_rgb(10, 132, 255),
        Color32::from_rgb(94, 92, 230),
        Color32::from_rgb(191, 90, 242),
        Color32::from_rgb(255, 55, 95),
        Color32::from_rgb(255, 159, 10),
        Color32::from_rgb(48, 176, 100),
        Color32::from_rgb(100, 210, 255),
        Color32::from_rgb(142, 142, 147),
    ];
    let h = title
        .bytes()
        .fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    p.rect_filled(rect, radius, COLORS[(h % 8) as usize]);
    let letter: String = title
        .trim()
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_else(|| "?".into());
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        letter,
        FontId::proportional(rect.width() * 0.5),
        Color32::WHITE,
    );
}

fn trunc_job(text: &str, size: f32, color: Color32, max_w: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::simple_singleline(
        text.to_owned(),
        FontId::proportional(size),
        color,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_w.max(10.0));
    job
}

// ───────────────────────────── widgets ─────────────────────────────

fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(12.5).color(pal::TEXT2));
}

fn input(ui: &mut egui::Ui, value: &mut String, hint: &str, secret: bool) -> egui::Response {
    ui.add(
        egui::TextEdit::singleline(value)
            .password(secret)
            .hint_text(RichText::new(hint).color(pal::TEXT3))
            .desired_width(f32::INFINITY)
            .margin(Margin::symmetric(12, 10))
            .min_size(vec2(0.0, 40.0))
            .vertical_align(Align::Center),
    )
}

fn primary_button_w(ui: &mut egui::Ui, text: &str, width: f32) -> egui::Response {
    ui.add_sized(
        [width, 40.0],
        egui::Button::new(RichText::new(text).size(14.5).color(Color32::WHITE))
            .fill(pal::ACCENT)
            .stroke(Stroke::NONE)
            .corner_radius(9),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn primary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let w = ui.available_width();
    primary_button_w(ui, text, w)
}

fn secondary_button_w(ui: &mut egui::Ui, text: &str, width: f32) -> egui::Response {
    ui.add_sized(
        [width, 40.0],
        egui::Button::new(RichText::new(text).size(14.5)).corner_radius(9),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn secondary_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let w = ui.available_width();
    secondary_button_w(ui, text, w)
}

fn link_button(ui: &mut egui::Ui, text: &str, color: Color32) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(text).size(13.5).color(color)).frame(false))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn enter_pressed(ui: &egui::Ui, resp: &egui::Response) -> bool {
    resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
}

fn app_icon(ui: &mut egui::Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, size * 0.23, pal::ACCENT);
    paint_lock(p, rect.center(), size * 0.62, Color32::WHITE, pal::ACCENT);
}

/// Centers a fixed-width column on screen; scrolls if the window is short.
fn auth_scaffold(ui: &mut egui::Ui, width: f32, est_height: f32, add: impl FnOnce(&mut egui::Ui)) {
    let avail = ui.available_height();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(((avail - est_height) * 0.5).max(24.0));
            ui.vertical_centered(|ui| {
                ui.allocate_ui_with_layout(
                    vec2(width, 0.0),
                    egui::Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_width(width);
                        add(ui);
                    },
                );
            });
            ui.add_space(24.0);
        });
}

fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(pal::CARD)
        .corner_radius(12)
        .inner_margin(Margin::same(18))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

fn section_label(ui: &mut egui::Ui, text: &str) {
    ui.add_space(14.0);
    ui.label(RichText::new(text).size(11.5).color(pal::TEXT3));
    ui.add_space(2.0);
}

fn nav_row(
    ui: &mut egui::Ui,
    icon: Icon,
    tint: Color32,
    label: &str,
    count: Option<usize>,
    selected: bool,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, 8.0, pal::ACCENT);
    } else if resp.hovered() {
        p.rect_filled(rect, 8.0, Color32::from_white_alpha(14));
    }
    let ic = Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(22.0, 22.0));
    p.rect_filled(ic, 6.0, tint);
    paint_icon(p, ic.center(), 22.0, icon, Color32::WHITE, tint);
    p.text(
        pos2(rect.left() + 40.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        pal::TEXT,
    );
    if let Some(n) = count {
        p.text(
            pos2(rect.right() - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            n.to_string(),
            FontId::proportional(12.5),
            if selected {
                Color32::from_white_alpha(200)
            } else {
                pal::TEXT3
            },
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

fn list_row(
    ui: &mut egui::Ui,
    row: &Row,
    selected: bool,
    tex: Option<&egui::TextureHandle>,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 58.0), Sense::click());
    let p = ui.painter();
    let inner = rect.shrink2(vec2(8.0, 2.0));
    if selected {
        p.rect_filled(inner, 9.0, pal::ACCENT);
    } else if resp.hovered() {
        p.rect_filled(inner, 9.0, Color32::from_white_alpha(12));
    }
    let ic = Rect::from_min_size(
        pos2(inner.left() + 9.0, inner.center().y - 18.0),
        vec2(36.0, 36.0),
    );
    paint_avatar(p, ic, &row.title, tex);
    let tx = ic.right() + 12.0;
    let max_w = inner.right() - tx - 34.0;
    let title_col = if selected { Color32::WHITE } else { pal::TEXT };
    let sub_col = if selected {
        Color32::from_white_alpha(205)
    } else {
        pal::TEXT2
    };
    let g = p.layout_job(trunc_job(&row.title, 14.5, title_col, max_w));
    p.galley(pos2(tx, inner.center().y - 19.0), g, title_col);
    let sub = if row.username.is_empty() {
        "No username"
    } else {
        row.username.as_str()
    };
    let g = p.layout_job(trunc_job(sub, 12.5, sub_col, max_w));
    p.galley(pos2(tx, inner.center().y + 2.0), g, sub_col);
    if row.favorite {
        paint_star(
            p,
            pos2(inner.right() - 18.0, inner.center().y),
            7.5,
            Some(if selected {
                Color32::WHITE
            } else {
                pal::YELLOW
            }),
            Stroke::NONE,
        );
    }
    if !selected && !resp.hovered() {
        p.hline(
            (tx)..=(rect.right() - 12.0),
            rect.bottom(),
            Stroke::new(1.0_f32, Color32::from_rgb(46, 46, 49)),
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn star_button(ui: &mut egui::Ui, filled: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, 8.0, Color32::from_white_alpha(16));
    }
    paint_star(
        p,
        rect.center(),
        9.5,
        if filled { Some(pal::YELLOW) } else { None },
        Stroke::new(1.6_f32, if filled { pal::YELLOW } else { pal::TEXT2 }),
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(if filled {
            "Remove from Favorites"
        } else {
            "Add to Favorites"
        })
}

fn plus_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::click());
    let p = ui.painter();
    p.circle_filled(
        rect.center(),
        15.0,
        if resp.hovered() {
            Color32::from_rgb(64, 156, 255)
        } else {
            pal::ACCENT
        },
    );
    let s = Stroke::new(2.0_f32, Color32::WHITE);
    p.line_segment(
        [
            rect.center() + vec2(-6.0, 0.0),
            rect.center() + vec2(6.0, 0.0),
        ],
        s,
    );
    p.line_segment(
        [
            rect.center() + vec2(0.0, -6.0),
            rect.center() + vec2(0.0, 6.0),
        ],
        s,
    );
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text("New password")
}

fn search_field(ui: &mut egui::Ui, value: &mut String) {
    let left = ui.available_rect_before_wrap().left();
    let resp = ui.add(
        egui::TextEdit::singleline(value)
            .hint_text(RichText::new("Search").color(pal::TEXT3))
            .desired_width(f32::INFINITY)
            .margin(Margin {
                left: 34,
                right: 10,
                top: 8,
                bottom: 8,
            })
            .min_size(vec2(0.0, 36.0))
            .vertical_align(Align::Center),
    );
    paint_search_icon(
        ui.painter(),
        pos2(left + 17.0, resp.rect.center().y),
        7.0,
        pal::TEXT3,
    );
}

fn detail_row(ui: &mut egui::Ui, label: &str, first: bool, body: impl FnOnce(&mut egui::Ui)) {
    if !first {
        let y = ui.cursor().top();
        let x = ui.available_rect_before_wrap().x_range();
        ui.painter()
            .hline(x.min + 16.0..=x.max, y, Stroke::new(1.0_f32, pal::SEP));
    }
    egui::Frame::new()
        .inner_margin(Margin::symmetric(16, 11))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.label(RichText::new(label).size(12.0).color(pal::TEXT2));
            body(ui);
        });
}

fn action_row(ui: &mut egui::Ui, title: &str, desc: &str, button: &str) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            clicked = secondary_button_w(ui, button, 150.0).clicked();
            ui.with_layout(egui::Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.label(RichText::new(title).size(15.0).color(pal::TEXT));
                ui.label(RichText::new(desc).size(12.5).color(pal::TEXT2));
            });
        });
    });
    clicked
}

// ───────────────────────────── screens ─────────────────────────────

impl PasswordManagerApp {
    fn status_line(&self, ui: &mut egui::Ui) {
        if self.status.text.is_empty() {
            return;
        }
        ui.add_space(10.0);
        let col = if self.status.ok { pal::GREEN } else { pal::RED };
        ui.label(RichText::new(&self.status.text).size(13.0).color(col));
    }

    fn ui_welcome(&mut self, ui: &mut egui::Ui) {
        auth_scaffold(ui, 380.0, 400.0, |ui| {
            ui.vertical_centered(|ui| {
                app_icon(ui, 92.0);
                ui.add_space(22.0);
                ui.label(RichText::new("Password Manager").size(32.0).color(pal::TEXT));
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Your passwords, encrypted and stored only on this device.")
                        .size(14.0)
                        .color(pal::TEXT2),
                );
            });
            ui.add_space(32.0);
            if primary_button(ui, "Create New Vault").clicked() {
                self.open_create();
            }
            if secondary_button(ui, "Open Existing Vault").clicked() {
                self.open_existing();
            }
            self.status_line(ui);
            ui.add_space(28.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("No account  ·  No cloud  ·  No tracking")
                        .size(12.0)
                        .color(pal::TEXT3),
                );
            });
        });
    }

    fn ui_create(&mut self, ui: &mut egui::Ui) {
        auth_scaffold(ui, 380.0, 500.0, |ui| {
            ui.vertical_centered(|ui| {
                app_icon(ui, 64.0);
                ui.add_space(16.0);
                ui.label(RichText::new("Create Vault").size(26.0).color(pal::TEXT));
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Choose a strong master password.")
                        .size(13.5)
                        .color(pal::TEXT2),
                );
            });
            ui.add_space(24.0);
            field_label(ui, "Master Password");
            input(
                ui,
                &mut self.master_password,
                "At least 12 characters",
                !self.show_password,
            );
            ui.add_space(6.0);
            field_label(ui, "Confirm Password");
            let r = input(
                ui,
                &mut self.confirm_password,
                "Re-enter password",
                !self.show_password,
            );
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.show_password, "Show passwords");
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let n = self.master_password.len();
                    let (txt, col) = if n >= MIN_MASTER_LEN {
                        ("Length OK".to_owned(), pal::GREEN)
                    } else {
                        (format!("{n} / {MIN_MASTER_LEN}"), pal::TEXT3)
                    };
                    ui.label(RichText::new(txt).size(12.5).color(col));
                });
            });
            self.status_line(ui);
            ui.add_space(14.0);
            if primary_button(ui, "Continue").clicked() || enter_pressed(ui, &r) {
                self.choose_new_vault();
            }
            if secondary_button(ui, "Back").clicked() {
                self.status.clear();
                self.screen = Screen::Welcome;
            }
        });
    }

    fn ui_recovery_confirm(&mut self, ui: &mut egui::Ui) {
        auth_scaffold(ui, 460.0, 620.0, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("Your Recovery Key")
                        .size(26.0)
                        .color(pal::TEXT),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Write it down and keep it somewhere safe, offline.")
                        .size(13.5)
                        .color(pal::TEXT2),
                );
            });
            ui.add_space(16.0);
            egui::Frame::new()
                .fill(Color32::from_rgb(58, 46, 24))
                .corner_radius(10)
                .inner_margin(Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(
                            "This key is the ONLY way back in if you forget your master password. If you lose both, the vault cannot be recovered.",
                        )
                        .size(13.0)
                        .color(pal::ORANGE),
                    );
                });
            ui.add_space(12.0);
            card(ui, |ui| {
                let groups: Vec<&str> = self.pending_recovery.split('-').collect();
                ui.vertical_centered(|ui| {
                    ui.spacing_mut().item_spacing.y = 6.0;
                    for chunk in groups.chunks(4) {
                        ui.label(
                            RichText::new(chunk.join("   "))
                                .monospace()
                                .size(17.0)
                                .color(Color32::WHITE),
                        );
                    }
                });
                ui.add_space(8.0);
                if secondary_button(ui, "Copy Recovery Key").clicked() {
                    let value = self.pending_recovery.clone();
                    self.copy_to_clipboard(&value);
                }
                ui.add_space(2.0);
                ui.label(
                    RichText::new(
                        "Clipboard history and managers may keep a copy of anything you copy.",
                    )
                    .size(12.0)
                    .color(pal::TEXT3),
                );
            });
            ui.add_space(14.0);
            field_label(ui, "Re-enter the key to confirm you saved it");
            let r = input(
                ui,
                &mut self.recovery_confirmation,
                "Paste or type your recovery key",
                false,
            );
            self.status_line(ui);
            ui.add_space(14.0);
            let label = if self.rotating_recovery {
                "Verify and Save New Key"
            } else {
                "Verify and Create Vault"
            };
            if primary_button(ui, label).clicked() || enter_pressed(ui, &r) {
                self.finish_creation();
            }
        });
    }

    fn ui_unlock(&mut self, ui: &mut egui::Ui) {
        if self.show_recover_form {
            self.ui_recover(ui);
            return;
        }
        auth_scaffold(ui, 380.0, 440.0, |ui| {
            let name = self
                .vault_path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "No vault selected".into());
            ui.vertical_centered(|ui| {
                app_icon(ui, 72.0);
                ui.add_space(16.0);
                ui.label(RichText::new("Unlock Vault").size(26.0).color(pal::TEXT));
                ui.add_space(2.0);
                ui.label(RichText::new(name).size(13.0).color(pal::TEXT2));
            });
            ui.add_space(24.0);
            field_label(ui, "Master Password");
            let r = input(
                ui,
                &mut self.master_password,
                "Enter master password",
                !self.show_password,
            );
            ui.add_space(2.0);
            ui.checkbox(&mut self.show_password, "Show password");
            self.status_line(ui);
            ui.add_space(12.0);
            if primary_button(ui, "Unlock").clicked() || enter_pressed(ui, &r) {
                self.unlock();
            }
            ui.add_space(2.0);
            ui.vertical_centered(|ui| {
                if link_button(ui, "Forgot Master Password?", pal::ACCENT).clicked() {
                    self.show_recover_form = true;
                    self.status.clear();
                }
            });
            ui.add_space(14.0);
            if secondary_button(ui, "Choose a Different Vault").clicked() {
                self.status.clear();
                self.screen = Screen::Welcome;
            }
        });
    }

    fn ui_recover(&mut self, ui: &mut egui::Ui) {
        auth_scaffold(ui, 380.0, 560.0, |ui| {
            ui.vertical_centered(|ui| {
                app_icon(ui, 56.0);
                ui.add_space(14.0);
                ui.label(RichText::new("Recover Vault").size(26.0).color(pal::TEXT));
                ui.add_space(2.0);
                ui.label(
                    RichText::new("Use your recovery key, then choose a new master password.")
                        .size(13.0)
                        .color(pal::TEXT2),
                );
            });
            ui.add_space(20.0);
            field_label(ui, "Recovery Key");
            input(
                ui,
                &mut self.recovery_key,
                "XXXXX-XXXXX-XXXXX-…",
                !self.show_recovery,
            );
            ui.add_space(2.0);
            ui.checkbox(&mut self.show_recovery, "Show recovery key");
            ui.add_space(6.0);
            field_label(ui, "New Master Password");
            input(
                ui,
                &mut self.new_master_password,
                "At least 12 characters",
                true,
            );
            ui.add_space(6.0);
            field_label(ui, "Confirm New Password");
            let r = input(
                ui,
                &mut self.confirm_password,
                "Re-enter new password",
                true,
            );
            self.status_line(ui);
            ui.add_space(14.0);
            if primary_button(ui, "Recover Vault").clicked() || enter_pressed(ui, &r) {
                self.recover();
            }
            if secondary_button(ui, "Back to Unlock").clicked() {
                self.show_recover_form = false;
                self.status.clear();
            }
        });
    }
}

// ───────────────────────────── dashboard ─────────────────────────────

impl PasswordManagerApp {
    fn dashboard(&mut self, ctx: &egui::Context) {
        let mut seps: Vec<f32> = Vec::new();
        let side = egui::SidePanel::left("sidebar")
            .exact_width(208.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(pal::SIDEBAR)
                    .inner_margin(Margin::symmetric(12, 16)),
            )
            .show(ctx, |ui| self.sidebar_ui(ui));
        seps.push(side.response.rect.right());
        if self.screen != Screen::Dashboard {
            // Locked from the sidebar: do not draw another frame of vault data.
            ctx.request_repaint();
            return;
        }
        if self.section != Section::Settings {
            let list = egui::SidePanel::left("list")
                .exact_width(300.0)
                .resizable(false)
                .frame(
                    egui::Frame::new()
                        .fill(pal::LIST)
                        .inner_margin(Margin::symmetric(12, 16)),
                )
                .show(ctx, |ui| self.list_ui(ui));
            seps.push(list.response.rect.right());
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(pal::BG)
                    .inner_margin(Margin::symmetric(28, 24)),
            )
            .show(ctx, |ui| {
                if self.section == Section::Settings {
                    self.settings_ui(ui);
                } else {
                    self.detail_ui(ui);
                }
            });
        let painter = ctx.layer_painter(egui::LayerId::background());
        for x in seps {
            painter.vline(
                x,
                ctx.screen_rect().y_range(),
                Stroke::new(1.0_f32, pal::SEP),
            );
        }
        self.entry_modal(ctx);
        self.delete_modal(ctx);
        self.toast(ctx);
    }

    fn sidebar_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::hover());
            ui.painter().rect_filled(r, 7.0, pal::ACCENT);
            paint_lock(ui.painter(), r.center(), 19.0, Color32::WHITE, pal::ACCENT);
            ui.label(
                RichText::new("Password Manager")
                    .size(16.0)
                    .color(pal::TEXT)
                    .strong(),
            );
        });
        ui.add_space(18.0);
        let (total, favs) = self
            .vault
            .as_ref()
            .map(|v| {
                (
                    v.data.entries.len(),
                    v.data.entries.iter().filter(|e| e.favorite).count(),
                )
            })
            .unwrap_or((0, 0));
        ui.spacing_mut().item_spacing.y = 2.0;
        let items = [
            (
                Section::All,
                Icon::Lock,
                pal::ACCENT,
                "All Passwords",
                Some(total),
            ),
            (
                Section::Favorites,
                Icon::Star,
                pal::ORANGE,
                "Favorites",
                Some(favs),
            ),
            (
                Section::Recent,
                Icon::Clock,
                Color32::from_rgb(48, 176, 100),
                "Recently Added",
                None,
            ),
        ];
        for (section, icon, tint, label, count) in items {
            if nav_row(ui, icon, tint, label, count, self.section == section) {
                self.section = section;
            }
        }
        ui.add_space(10.0);
        let y = ui.cursor().top();
        ui.painter()
            .hline(ui.max_rect().x_range(), y, Stroke::new(1.0_f32, pal::SEP));
        ui.add_space(10.0);
        if nav_row(
            ui,
            Icon::Gear,
            Color32::from_rgb(142, 142, 147),
            "Settings",
            None,
            self.section == Section::Settings,
        ) {
            self.section = Section::Settings;
        }
        ui.with_layout(egui::Layout::bottom_up(Align::Min), |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            if secondary_button(ui, "Lock Vault").clicked() {
                self.lock();
            }
            ui.label(
                RichText::new("Offline  ·  Encrypted on this device")
                    .size(11.5)
                    .color(pal::TEXT3),
            );
        });
    }

    fn list_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let title = match self.section {
            Section::All => "All Passwords",
            Section::Favorites => "Favorites",
            Section::Recent => "Recently Added",
            Section::Settings => "",
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).size(20.0).color(pal::TEXT).strong());
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if plus_button(ui).clicked() {
                    self.new_entry();
                }
            });
        });
        ui.add_space(6.0);
        search_field(ui, &mut self.search);
        ui.add_space(8.0);

        let rows = self.rows();
        for r in &rows {
            if r.has_icon {
                self.ensure_icon(&ctx, r.id);
            }
        }
        let total = self.vault.as_ref().map_or(0, |v| v.data.entries.len());
        if rows.is_empty() {
            ui.add_space(60.0);
            ui.vertical_centered(|ui| {
                let (head, sub) = if total == 0 {
                    (
                        "No Passwords",
                        "Click the + button to add your first password.",
                    )
                } else if self.section == Section::Favorites && self.search.is_empty() {
                    ("No Favorites", "Tap the star on a password to add it here.")
                } else {
                    ("No Results", "Try a different search.")
                };
                ui.label(RichText::new(head).size(17.0).color(pal::TEXT2));
                ui.label(RichText::new(sub).size(12.5).color(pal::TEXT3));
            });
            return;
        }
        let mut clicked = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for row in &rows {
                    let tex = self.icons.get(&row.id).and_then(|o| o.as_ref());
                    if list_row(ui, row, self.selected == Some(row.id), tex).clicked() {
                        clicked = Some(row.id);
                    }
                }
            });
        if let Some(id) = clicked {
            self.selected = Some(id);
            self.reveal = false;
        }
    }

    fn empty_detail(&self, ui: &mut egui::Ui) {
        let avail = ui.available_height();
        ui.add_space((avail * 0.36).max(40.0));
        ui.vertical_centered(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(64.0, 64.0), Sense::hover());
            let c = Color32::from_rgb(70, 70, 76);
            ui.painter().rect_filled(rect, 15.0, c);
            paint_lock(ui.painter(), rect.center(), 40.0, pal::BG, c);
            ui.add_space(10.0);
            ui.label(
                RichText::new("No Password Selected")
                    .size(17.0)
                    .color(pal::TEXT2),
            );
        });
    }

    fn detail_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let Some(id) = self.selected else {
            self.empty_detail(ui);
            return;
        };
        struct View {
            title: String,
            username: String,
            url: String,
            notes: String,
            favorite: bool,
            created: u64,
            modified: u64,
            has_icon: bool,
        }
        let view = self
            .vault
            .as_ref()
            .and_then(|v| v.data.entries.iter().find(|e| e.id == id))
            .map(|e| View {
                title: e.title.clone(),
                username: e.username.clone(),
                url: e.url.clone(),
                notes: e.notes.clone(),
                favorite: e.favorite,
                created: e.created_unix,
                modified: e.modified_unix,
                has_icon: e.icon_bytes.is_some(),
            });
        let Some(view) = view else {
            self.selected = None;
            self.empty_detail(ui);
            return;
        };
        if view.has_icon {
            self.ensure_icon(&ctx, id);
        }
        // The password is only copied into the frame when the user reveals it,
        // and the copy is wiped when the frame ends.
        let revealed: Option<Zeroizing<String>> = if self.reveal {
            self.vault
                .as_ref()
                .and_then(|v| v.data.entries.iter().find(|e| e.id == id))
                .map(|e| Zeroizing::new(e.password.clone()))
        } else {
            None
        };
        let mut action: Option<DetailAction> = None;
        let tex = self.icons.get(&id).and_then(|o| o.as_ref());

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(76.0, 76.0), Sense::hover());
                    paint_avatar(ui.painter(), r, &view.title, tex);
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.add_space(8.0);
                        ui.spacing_mut().item_spacing.y = 3.0;
                        ui.label(RichText::new(&view.title).size(26.0).color(pal::TEXT));
                        ui.label(
                            RichText::new(if view.url.is_empty() {
                                "Password"
                            } else {
                                &view.url
                            })
                            .size(13.5)
                            .color(pal::TEXT2),
                        );
                    });
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if star_button(ui, view.favorite).clicked() {
                        action = Some(DetailAction::ToggleFavorite);
                    }
                    if secondary_button_w(ui, "Edit", 84.0).clicked() {
                        action = Some(DetailAction::Edit);
                    }
                    if ui
                        .add_sized(
                            [84.0, 40.0],
                            egui::Button::new(RichText::new("Delete").size(14.5).color(pal::RED))
                                .corner_radius(9),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        action = Some(DetailAction::Delete);
                    }
                });
                ui.add_space(16.0);

                egui::Frame::new()
                    .fill(pal::CARD)
                    .corner_radius(12)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = 0.0;
                        detail_row(ui, "User Name", true, |ui| {
                            ui.horizontal(|ui| {
                                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                                    if !view.username.is_empty()
                                        && link_button(ui, "Copy", pal::ACCENT).clicked()
                                    {
                                        action = Some(DetailAction::CopyUser);
                                    }
                                    ui.with_layout(
                                        egui::Layout::left_to_right(Align::Center),
                                        |ui| {
                                            let (t, c) = if view.username.is_empty() {
                                                ("Not set", pal::TEXT3)
                                            } else {
                                                (view.username.as_str(), pal::TEXT)
                                            };
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(t).size(15.0).color(c),
                                                )
                                                .truncate(),
                                            );
                                        },
                                    );
                                });
                            });
                        });
                        detail_row(ui, "Password", false, |ui| {
                            ui.horizontal(|ui| {
                                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                                    if link_button(ui, "Copy", pal::ACCENT).clicked() {
                                        action = Some(DetailAction::CopyPass);
                                    }
                                    if link_button(
                                        ui,
                                        if self.reveal { "Hide" } else { "Show" },
                                        pal::ACCENT,
                                    )
                                    .clicked()
                                    {
                                        action = Some(DetailAction::ToggleReveal);
                                    }
                                    ui.with_layout(
                                        egui::Layout::left_to_right(Align::Center),
                                        |ui| {
                                            match &revealed {
                                                Some(pw) => ui.add(
                                                    egui::Label::new(
                                                        RichText::new(pw.as_str())
                                                            .monospace()
                                                            .size(15.0)
                                                            .color(pal::TEXT),
                                                    )
                                                    .truncate(),
                                                ),
                                                None => ui.label(
                                                    RichText::new("••••••••••••")
                                                        .size(15.0)
                                                        .color(pal::TEXT),
                                                ),
                                            };
                                        },
                                    );
                                });
                            });
                        });
                        if !view.url.is_empty() {
                            detail_row(ui, "Website", false, |ui| {
                                ui.horizontal(|ui| {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(Align::Center),
                                        |ui| {
                                            if link_button(ui, "Copy", pal::ACCENT).clicked() {
                                                action = Some(DetailAction::CopyUrl);
                                            }
                                            ui.with_layout(
                                                egui::Layout::left_to_right(Align::Center),
                                                |ui| {
                                                    ui.add(
                                                        egui::Label::new(
                                                            RichText::new(&view.url)
                                                                .size(15.0)
                                                                .color(pal::TEXT),
                                                        )
                                                        .truncate(),
                                                    );
                                                },
                                            );
                                        },
                                    );
                                });
                            });
                        }
                        if !view.notes.is_empty() {
                            detail_row(ui, "Notes", false, |ui| {
                                ui.label(RichText::new(&view.notes).size(14.5).color(pal::TEXT));
                            });
                        }
                    });
                ui.add_space(14.0);
                ui.label(
                    RichText::new(format!(
                        "Created {}   ·   Modified {}",
                        fmt_time(view.created),
                        fmt_time(view.modified)
                    ))
                    .size(12.0)
                    .color(pal::TEXT3),
                );
            });

        match action {
            Some(DetailAction::CopyUser) => self.copy_to_clipboard(&view.username),
            Some(DetailAction::CopyUrl) => self.copy_to_clipboard(&view.url),
            Some(DetailAction::CopyPass) => {
                let pw = self
                    .vault
                    .as_ref()
                    .and_then(|v| v.data.entries.iter().find(|e| e.id == id))
                    .map(|e| Zeroizing::new(e.password.clone()));
                if let Some(pw) = pw {
                    self.copy_to_clipboard(&pw);
                }
            }
            Some(DetailAction::ToggleReveal) => self.reveal = !self.reveal,
            Some(DetailAction::ToggleFavorite) => self.toggle_favorite(id),
            Some(DetailAction::Edit) => self.edit_selected(&ctx),
            Some(DetailAction::Delete) => self.confirm_delete = Some(id),
            None => {}
        }
    }

    fn generator_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Length").size(13.0).color(pal::TEXT2));
            ui.add(egui::Slider::new(&mut self.generator_length, 12..=64));
        });
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.generator_upper, "A-Z");
            ui.checkbox(&mut self.generator_lower, "a-z");
            ui.checkbox(&mut self.generator_digits, "0-9");
            ui.checkbox(&mut self.generator_symbols, "Symbols");
        });
    }

    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        let width = ui.available_width().min(580.0);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.allocate_ui_with_layout(
                        vec2(width, 0.0),
                        egui::Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(width);
                            self.settings_body(ui);
                        },
                    );
                });
            });
    }

    fn settings_body(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Settings").size(26.0).color(pal::TEXT));

        section_label(ui, "MASTER PASSWORD");
        card(ui, |ui| {
            ui.label(
                RichText::new("Change Master Password")
                    .size(15.0)
                    .color(pal::TEXT),
            );
            ui.label(
                RichText::new("Your recovery key keeps working after the change.")
                    .size(12.5)
                    .color(pal::TEXT2),
            );
            ui.add_space(8.0);
            field_label(ui, "Current Password");
            input(
                ui,
                &mut self.current_password,
                "Current master password",
                !self.show_password,
            );
            ui.add_space(4.0);
            field_label(ui, "New Password");
            input(
                ui,
                &mut self.new_master_password,
                "At least 12 characters",
                !self.show_password,
            );
            ui.add_space(4.0);
            field_label(ui, "Confirm New Password");
            input(
                ui,
                &mut self.confirm_password,
                "Re-enter new password",
                !self.show_password,
            );
            ui.add_space(2.0);
            ui.checkbox(&mut self.show_password, "Show passwords");
            ui.add_space(6.0);
            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                if primary_button_w(ui, "Change Password", 170.0).clicked() {
                    self.change_master();
                }
            });
        });

        section_label(ui, "RECOVERY");
        card(ui, |ui| {
            if self.confirm_rotate {
                ui.label(
                    RichText::new("Replace Recovery Key?")
                        .size(15.0)
                        .color(pal::TEXT),
                );
                ui.label(
                    RichText::new(
                        "Your current recovery key stops working immediately. You must write down and verify the new one.",
                    )
                    .size(12.5)
                    .color(pal::ORANGE),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if primary_button_w(ui, "Replace Key", 150.0).clicked() {
                        self.replace_recovery_key();
                    }
                    if secondary_button_w(ui, "Cancel", 110.0).clicked() {
                        self.confirm_rotate = false;
                    }
                });
            } else if action_row(
                ui,
                "Replace Recovery Key",
                "Generate a new key and invalidate the old one.",
                "Replace…",
            ) {
                self.confirm_rotate = true;
            }
        });

        section_label(ui, "BACKUP");
        card(ui, |ui| {
            if action_row(
                ui,
                "Export Encrypted Backup",
                "An encrypted copy of this vault. It needs the same master password or recovery key.",
                "Export…",
            ) {
                self.export_backup();
            }
        });

        section_label(ui, "PASSWORD GENERATOR");
        card(ui, |ui| {
            self.generator_controls(ui);
            ui.add_space(4.0);
            if secondary_button(ui, "Generate Password").clicked() {
                self.generate_password();
            }
            if !self.generated.is_empty() {
                ui.add_space(4.0);
                egui::Frame::new()
                    .fill(pal::FIELD)
                    .corner_radius(8)
                    .inner_margin(Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(
                            egui::Label::new(
                                RichText::new(&self.generated)
                                    .monospace()
                                    .size(15.0)
                                    .color(pal::TEXT),
                            )
                            .wrap(),
                        );
                    });
                if secondary_button(ui, "Copy Generated Password").clicked() {
                    let value = Zeroizing::new(self.generated.clone());
                    self.copy_to_clipboard(&value);
                }
            }
        });

        section_label(ui, "ABOUT THIS VAULT");
        card(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            for (k, v) in [
                ("Encryption", "XChaCha20-Poly1305"),
                ("Key derivation", "Argon2id (19 MiB, 2 passes)"),
                ("Auto-lock", "After 5 minutes of inactivity"),
                ("Clipboard", "Auto-cleared after 30 seconds"),
                ("Network", "None. Fully offline."),
            ] {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(k).size(13.5).color(pal::TEXT2));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(v).size(13.5).color(pal::TEXT));
                    });
                });
            }
        });
        ui.add_space(24.0);
    }

    fn entry_modal(&mut self, ctx: &egui::Context) {
        if !self.adding {
            return;
        }
        let editing = self.edit_id.is_some();
        let frame = egui::Frame::new()
            .fill(pal::CARD)
            .stroke(Stroke::new(1.0_f32, pal::SEP))
            .corner_radius(16)
            .inner_margin(Margin::same(22));
        egui::Modal::new(egui::Id::new("entry_form"))
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_width(440.0);
                ui.label(
                    RichText::new(if editing {
                        "Edit Password"
                    } else {
                        "New Password"
                    })
                    .size(20.0)
                    .color(pal::TEXT),
                );
                ui.add_space(8.0);
                let max_h = (ctx.screen_rect().height() - 200.0).max(240.0);
                egui::ScrollArea::vertical()
                    .max_height(max_h)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.set_width(440.0);
                        ui.set_max_width(440.0);
                        ui.spacing_mut().item_spacing.y = 5.0;
                        field_label(ui, "Title");
                        input(ui, &mut self.form.title, "e.g. GitHub", false);
                        ui.add_space(6.0);
                        field_label(ui, "User Name / Email");
                        input(ui, &mut self.form.username, "Optional", false);
                        ui.add_space(6.0);
                        field_label(ui, "Password");
                        ui.horizontal(|ui| {
                            let w = 440.0 - 24.0 - 76.0;
                            ui.add(
                                egui::TextEdit::singleline(&mut self.form.password)
                                    .password(!self.form.show)
                                    .hint_text(RichText::new("Required").color(pal::TEXT3))
                                    .desired_width(w)
                                    .margin(Margin::symmetric(12, 10))
                                    .min_size(vec2(0.0, 40.0))
                                    .vertical_align(Align::Center),
                            );
                            let label = if self.form.show { "Hide" } else { "Show" };
                            if ui
                                .add_sized([66.0, 40.0], egui::Button::new(label))
                                .clicked()
                            {
                                self.form.show = !self.form.show;
                            }
                        });
                        egui::CollapsingHeader::new(
                            RichText::new("Password generator")
                                .size(13.0)
                                .color(pal::ACCENT),
                        )
                        .show(ui, |ui| {
                            self.generator_controls(ui);
                            if secondary_button(ui, "Generate and Use").clicked() {
                                self.generate_password();
                                if !self.generated.is_empty() {
                                    self.form.password.zeroize();
                                    self.form.password = self.generated.clone();
                                    self.form.show = true;
                                }
                            }
                        });
                        ui.add_space(6.0);
                        field_label(ui, "Website");
                        input(ui, &mut self.form.url, "Optional", false);
                        ui.add_space(6.0);
                        field_label(ui, "Notes");
                        ui.add(
                            egui::TextEdit::multiline(&mut self.form.notes)
                                .hint_text(RichText::new("Optional").color(pal::TEXT3))
                                .desired_width(416.0)
                                .desired_rows(3)
                                .margin(Margin::symmetric(12, 10)),
                        );
                        ui.add_space(8.0);
                        field_label(ui, "Icon");
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::hover());
                            paint_avatar(
                                ui.painter(),
                                r,
                                &self.form.title,
                                self.form_icon_tex.as_ref(),
                            );
                            if secondary_button_w(
                                ui,
                                if self.form.icon_bytes.is_some() {
                                    "Replace…"
                                } else {
                                    "Choose Image…"
                                },
                                130.0,
                            )
                            .clicked()
                            {
                                self.choose_icon(ctx);
                            }
                            if self.form.icon_bytes.is_some()
                                && secondary_button_w(ui, "Remove", 90.0).clicked()
                            {
                                if let Some(old) = self.form.icon_bytes.as_mut() {
                                    old.zeroize();
                                }
                                self.form.icon_bytes = None;
                                self.form.icon_mime = None;
                                self.form_icon_tex = None;
                            }
                        });
                        ui.label(
                            RichText::new("Icons are stored encrypted inside the vault.")
                                .size(12.0)
                                .color(pal::TEXT3),
                        );
                        ui.add_space(8.0);
                        ui.checkbox(&mut self.form.favorite, "Add to Favorites");
                    });
                self.status_line(ui);
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if primary_button_w(ui, "Save", 130.0).clicked() {
                        self.save_entry();
                    }
                    if secondary_button_w(ui, "Cancel", 110.0).clicked() {
                        self.cancel_form();
                    }
                });
            });
        if self.adding && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cancel_form();
        }
    }

    fn delete_modal(&mut self, ctx: &egui::Context) {
        let Some(id) = self.confirm_delete else {
            return;
        };
        let title = self
            .vault
            .as_ref()
            .and_then(|v| v.data.entries.iter().find(|e| e.id == id))
            .map(|e| e.title.clone());
        let Some(title) = title else {
            self.confirm_delete = None;
            return;
        };
        let frame = egui::Frame::new()
            .fill(pal::CARD)
            .stroke(Stroke::new(1.0_f32, pal::SEP))
            .corner_radius(16)
            .inner_margin(Margin::same(22));
        egui::Modal::new(egui::Id::new("confirm_delete"))
            .frame(frame)
            .show(ctx, |ui| {
                ui.set_width(340.0);
                ui.label(
                    RichText::new(format!("Delete \"{title}\"?"))
                        .size(18.0)
                        .color(pal::TEXT),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new("This permanently removes the password from your vault.")
                        .size(13.0)
                        .color(pal::TEXT2),
                );
                ui.add_space(16.0);
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let del = ui
                        .add_sized(
                            [110.0, 40.0],
                            egui::Button::new(
                                RichText::new("Delete").size(14.5).color(Color32::WHITE),
                            )
                            .fill(pal::RED)
                            .stroke(Stroke::NONE)
                            .corner_radius(9),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                    if del.clicked() {
                        self.confirm_delete = None;
                        self.delete_entry(id);
                    }
                    if secondary_button_w(ui, "Cancel", 110.0).clicked() {
                        self.confirm_delete = None;
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.confirm_delete = None;
        }
    }

    fn toast(&self, ctx: &egui::Context) {
        if self.status.text.is_empty() || self.adding || self.confirm_delete.is_some() {
            return;
        }
        let col = if self.status.ok { pal::GREEN } else { pal::RED };
        egui::Area::new(egui::Id::new("toast"))
            .order(egui::Order::Tooltip)
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -20.0))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgb(58, 58, 63))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(84, 84, 90)))
                    .corner_radius(18)
                    .inner_margin(Margin::symmetric(16, 9))
                    .show(ui, |ui| {
                        ui.set_max_width(560.0);
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 4.0, col);
                            ui.label(RichText::new(&self.status.text).size(13.0).color(pal::TEXT));
                        });
                    });
            });
    }
}

// ───────────────────────────── frame loop ─────────────────────────────

impl eframe::App for PasswordManagerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.startup_size_adjusted {
            let (monitor_size, outer_rect, inner_rect) = ctx.input(|input| {
                let viewport = input.viewport();
                (
                    viewport.monitor_size,
                    viewport.outer_rect,
                    viewport.inner_rect,
                )
            });
            if let Some(monitor) = monitor_size {
                // Medium 3:2 landscape window, bounded by this monitor so the
                // taskbar and window decorations always fit.
                let height = (monitor.y * 0.86 - 24.0).clamp(600.0, 760.0);
                let width = (height * 1.5).min(monitor.x * 0.84).clamp(920.0, 1140.0);
                let size = vec2(width, height);
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
                if let Some(outer) = outer_rect {
                    let decorations = inner_rect
                        .map(|inner| outer.size() - inner.size())
                        .unwrap_or(egui::Vec2::ZERO);
                    let target_outer = size + decorations;
                    ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(
                        outer.center() - target_outer * 0.5,
                    ));
                }
                self.startup_size_adjusted = true;
            }
        }

        if ctx.input(|i| {
            i.pointer.any_down()
                || i.key_pressed(egui::Key::Enter)
                || i.key_pressed(egui::Key::Escape)
                || i.events.iter().any(|e| matches!(e, egui::Event::Text(_)))
        }) {
            self.last_activity = Instant::now();
        }
        if self.screen == Screen::Dashboard && self.last_activity.elapsed() > AUTO_LOCK {
            self.lock();
        }
        self.tick_clipboard();

        // Toasts disappear on their own after a few seconds.
        if self.status != self.status_seen {
            self.status_seen = self.status.clone();
            self.status_since = Instant::now();
        }
        if self.screen == Screen::Dashboard
            && !self.adding
            && !self.status.text.is_empty()
            && self.status_since.elapsed() > Duration::from_secs(6)
        {
            self.status.clear();
        }

        if self.screen == Screen::Dashboard {
            self.dashboard(ctx);
        } else {
            let screen = self.screen;
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::new()
                        .fill(pal::BG)
                        .inner_margin(Margin::symmetric(24, 12)),
                )
                .show(ctx, |ui| match screen {
                    Screen::Welcome => self.ui_welcome(ui),
                    Screen::Create => self.ui_create(ui),
                    Screen::RecoveryConfirm => self.ui_recovery_confirm(ui),
                    Screen::Unlock => self.ui_unlock(ui),
                    Screen::Dashboard => {}
                });
        }
        ctx.request_repaint_after(Duration::from_secs(1));
    }
}
