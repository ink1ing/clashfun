use anyhow::Result;
use sysinfo::{PidExt, ProcessExt, System, SystemExt};

#[derive(Debug, Clone)]
pub struct GameProcess {
    pub name: String,
    pub pid: u32,
    pub exe_path: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SupportedGame {
    DontStarveTogether,
}

impl SupportedGame {
    pub fn process_names(&self) -> Vec<&'static str> {
        match self {
            Self::DontStarveTogether => vec![
                "dontstarve_steam",
                "dontstarve_dedicated_server_nullrenderer",
                "Don't Starve Together",
            ],
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::DontStarveTogether => "饥荒联机版",
        }
    }
}

pub struct GameDetector {
    system: System,
    supported_games: Vec<SupportedGame>,
}

impl GameDetector {
    pub fn new() -> Self {
        Self {
            system: System::new_all(),
            supported_games: vec![SupportedGame::DontStarveTogether],
        }
    }

    pub fn refresh(&mut self) {
        self.system.refresh_processes();
    }

    pub fn detect_running_games(&mut self) -> Result<Vec<(SupportedGame, GameProcess)>> {
        self.refresh();

        let mut detected_games = Vec::new();

        for game in &self.supported_games {
            if let Some(process) = self.find_game_process(game)? {
                detected_games.push((game.clone(), process));
            }
        }

        Ok(detected_games)
    }

    fn find_game_process(&self, game: &SupportedGame) -> Result<Option<GameProcess>> {
        let process_names = game.process_names();

        for (pid, process) in self.system.processes() {
            let process_name = process.name();
            let exe_path = Some(process.exe().to_string_lossy().to_string());

            for &target_name in &process_names {
                if process_name.to_lowercase().contains(&target_name.to_lowercase()) {
                    return Ok(Some(GameProcess {
                        name: process_name.to_string(),
                        pid: pid.as_u32(),
                        exe_path,
                    }));
                }
            }

            if let Some(path) = &exe_path {
                for &target_name in &process_names {
                    if path.to_lowercase().contains(&target_name.to_lowercase()) {
                        return Ok(Some(GameProcess {
                            name: process_name.to_string(),
                            pid: pid.as_u32(),
                            exe_path,
                        }));
                    }
                }
            }
        }

        Ok(None)
    }

    pub fn is_game_running(&mut self, game: &SupportedGame) -> Result<bool> {
        Ok(self.find_game_process(game)?.is_some())
    }
}