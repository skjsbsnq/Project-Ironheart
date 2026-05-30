use winit::window::{Fullscreen, Window};

pub fn apply_command(
    settings: &mut hoi4_ui::settings::Settings,
    settings_panel: &mut hoi4_ui::settings::SettingsPanel,
    music_player: &mut hoi4_audio::MusicPlayer,
    ui_sounds: &mut hoi4_audio::UiSoundBank,
    window: &Window,
    cmd: hoi4_ui::settings::SettingsCommand,
) {
    use hoi4_ui::settings::SettingsCommand;
    match cmd {
        SettingsCommand::SetFullscreen(fs) => {
            settings.fullscreen = fs;
            let next = if fs {
                Some(Fullscreen::Borderless(None))
            } else {
                None
            };
            window.set_fullscreen(next);
        }
        SettingsCommand::SetResolution(w, h) => {
            settings.resolution = Some((w, h));
            if !settings.fullscreen {
                let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(w, h));
            }
        }
        SettingsCommand::SetVolumes => {
            settings.master_volume = settings_panel.draft.master_volume;
            settings.music_volume = settings_panel.draft.music_volume;
            settings.ui_volume = settings_panel.draft.ui_volume;
            let vols = hoi4_audio::AudioVolumes {
                master: settings.master_volume,
                music: settings.music_volume,
                ui: settings.ui_volume,
            };
            music_player.set_volumes(vols);
            ui_sounds.set_volumes(vols);
        }
        SettingsCommand::SetMaxSpeed(n) => {
            settings.max_speed = n;
        }
        SettingsCommand::SetLanguage(lang) => {
            settings.language = lang;
            hoi4_ui::i18n::set_language(lang);
        }
        SettingsCommand::SetAccessibility(accessibility) => {
            settings.color_blind_mode = accessibility.color_blind_mode;
            settings.font_scale = accessibility.font_scale;
        }
        SettingsCommand::SetEnable3dTerrain(b) => {
            settings.enable_3d_terrain = b;
            println!("[settings] enable_3d_terrain = {b}");
        }
        SettingsCommand::SetShowAllUnits(b) => {
            settings.show_all_units = b;
            println!("[debug] show_all_units = {b}");
        }
        SettingsCommand::SetHideAiFrontlines(b) => {
            settings.hide_ai_frontlines = b;
            println!("[debug] hide_ai_frontlines = {b}");
        }
        SettingsCommand::SetInstantWar(b) => {
            settings.instant_war = b;
            println!("[debug] instant_war = {b}");
        }
        SettingsCommand::Save => {
            *settings = settings_panel.draft.clone();
            if let Err(e) = settings.save() {
                settings_panel.last_error = Some(format!("save failed: {e}"));
            }
        }
    }
}
