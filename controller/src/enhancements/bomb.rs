use cs2::{
    PlantedC4,
    PlantedC4State,
};

use super::Enhancement;
use crate::{
    settings::AppSettings,
    utils::ImguiUiEx,
};
pub struct BombInfoIndicator {}

impl BombInfoIndicator {
    pub fn new() -> Self {
        Self {}
    }
}

/// % of the screens height
const PLAYER_AVATAR_TOP_OFFSET: f32 = 0.004;

/// % of the screens height
const PLAYER_AVATAR_SIZE: f32 = 0.05;

impl Enhancement for BombInfoIndicator {
    fn update(&mut self, _ctx: &crate::UpdateContext) -> anyhow::Result<()> {
        Ok(())
    }

    fn render(&self, states: &utils_state::StateRegistry, ui: &imgui::Ui) -> anyhow::Result<()> {
        let settings = states.resolve::<AppSettings>(())?;
        if !settings.bomb_timer {
            return Ok(());
        }

        let bomb_state = states.resolve::<PlantedC4>(())?;
        if matches!(bomb_state.state, PlantedC4State::NotPlanted) {
            return Ok(());
        }

        let group = ui.begin_group();

        let line_count = match &bomb_state.state {
            PlantedC4State::Active { .. } => 3,
            PlantedC4State::Defused | PlantedC4State::Detonated => 2,
            PlantedC4State::NotPlanted => unreachable!(),
        };
        let text_height = ui.text_line_height_with_spacing() * line_count as f32;

        /* align to be on the right side after the players */
        let offset_x = ui.io().display_size[0] * 1730.0 / 2560.0;
        let offset_y = ui.io().display_size[1] * PLAYER_AVATAR_TOP_OFFSET;
        let offset_y = offset_y
            + 0_f32.max((ui.io().display_size[1] * PLAYER_AVATAR_SIZE - text_height) / 2.0);

        ui.set_cursor_pos([offset_x, offset_y]);
        ui.text(&format!(
            "Bomb planted {}",
            if bomb_state.bomb_site == 0 { "A" } else { "B" }
        ));

        match &bomb_state.state {
            PlantedC4State::Active { time_detonation } => {
                ui.set_cursor_pos_x(offset_x);
                ui.text(&format!("Time: {:.3}", time_detonation));
                if let Some(defuser) = &bomb_state.defuser {
                    let color = if defuser.time_remaining > *time_detonation {
                        [0.79, 0.11, 0.11, 1.0]
                    } else {
                        [0.11, 0.79, 0.26, 1.0]
                    };

                    ui.set_cursor_pos_x(offset_x);
                    ui.text_colored(
                        color,
                        &format!(
                            "Defused in {:.3} by {}",
                            defuser.time_remaining, defuser.player_name
                        ),
                    );
                } else {
                    ui.set_cursor_pos_x(offset_x);
                    ui.text("Not defusing");
                }
            }
            PlantedC4State::Defused => {
                ui.set_cursor_pos_x(offset_x);
                ui.text("Bomb has been defused");
            }
            PlantedC4State::Detonated => {
                ui.set_cursor_pos_x(offset_x);
                ui.text("Bomb has been detonated");
            }
            PlantedC4State::NotPlanted => unreachable!(),
        }

        group.end();
        Ok(())
    }
}
