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
    CounterStrike,
    Dota2,
    LeagueOfLegends,
    Valorant,
    Minecraft,
    ApexLegends,
    Overwatch,
}

impl SupportedGame {
    pub fn process_names(&self) -> Vec<&'static str> {
        match self {
            Self::DontStarveTogether => vec![
                "dontstarve_steam",
                "dontstarve_dedicated_server_nullrenderer",
                "Don't Starve Together",
            ],
            Self::CounterStrike => vec![
                "cs2",
                "csgo",
                "Counter-Strike",
            ],
            Self::Dota2 => vec![
                "dota2",
                "Dota 2",
            ],
            Self::LeagueOfLegends => vec![
                "League of Legends",
                "LeagueClient",
                "RiotClientServices",
            ],
            Self::Valorant => vec![
                "VALORANT",
                "RiotClientServices",
            ],
            Self::Minecraft => vec![
                "minecraft",
                "javaw",
                "Minecraft",
            ],
            Self::ApexLegends => vec![
                "r5apex",
                "Apex Legends",
            ],
            Self::Overwatch => vec![
                "Overwatch",
                "OverwatchLauncher",
            ],
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::DontStarveTogether => "饥荒联机版",
            Self::CounterStrike => "反恐精英",
            Self::Dota2 => "刀塔2",
            Self::LeagueOfLegends => "英雄联盟",
            Self::Valorant => "无畏契约",
            Self::Minecraft => "我的世界",
            Self::ApexLegends => "Apex英雄",
            Self::Overwatch => "守望先锋",
        }
    }

    pub fn get_game_ports(&self) -> Vec<u16> {
        match self {
            Self::DontStarveTogether => vec![10999, 11000, 12346, 12347],
            Self::CounterStrike => vec![27015, 27005, 27020],
            Self::Dota2 => vec![27015, 27005, 27020],
            Self::LeagueOfLegends => vec![2099, 5223, 5222, 8393, 8394],
            Self::Valorant => vec![7777, 7778, 7779, 7780],
            Self::Minecraft => vec![25565, 25566, 25567],
            Self::ApexLegends => vec![37015, 37020],
            Self::Overwatch => vec![1119, 3724, 6113, 12000],
        }
    }

    pub fn should_optimize(&self) -> bool {
        match self {
            Self::DontStarveTogether => true,
            Self::CounterStrike => true,
            Self::Dota2 => true,
            Self::LeagueOfLegends => true,
            Self::Valorant => true,
            Self::Minecraft => true,
            Self::ApexLegends => true,
            Self::Overwatch => true,
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
            supported_games: vec![
                SupportedGame::DontStarveTogether,
                SupportedGame::CounterStrike,
                SupportedGame::Dota2,
                SupportedGame::LeagueOfLegends,
                SupportedGame::Valorant,
                SupportedGame::Minecraft,
                SupportedGame::ApexLegends,
                SupportedGame::Overwatch,
            ],
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