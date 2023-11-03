use std::ffi::CStr;
use std::time::Instant;

use anyhow::Context;
use cs2::CEntityIdentityEx;
use cs2_schema_declaration::Ptr;
use cs2_schema_generated::cs2::client::{
    CCSPlayer_ItemServices,
    CSkeletonInstance,
    C_CSPlayerPawn,
};
use obfstr::obfstr;
use serde::Serialize;

use super::Enhancement;
use crate::{
    settings::AppSettings,
    view::ViewController,
    weapon::WeaponId,
    web_radar_server::{
        MessageData,
        CLIENTS,
    },
};

#[derive(Serialize)]
pub struct WebPlayerInfo {
    pub controller_entity_id: u32,
    pub team_id: u8,

    pub player_health: i32,
    pub player_has_defuser: bool,
    pub player_name: String,
    pub weapon: WeaponId,
    pub player_flashtime: f32,

    pub position: [f32; 3],
}

#[derive(Serialize)]
pub struct WebPlayersInfo {
    pub type_name: &'static str,
    pub players: Vec<WebPlayerInfo>,
}

impl WebPlayersInfo {
    pub fn new(players: Vec<WebPlayerInfo>) -> Self {
        Self {
            type_name: "WebPlayersInfo",
            players,
        }
    }
}

pub struct WebRadar {
    players_info: WebPlayersInfo,
    timestamp: Instant,
}

const UPDATE_TIME:u128 = 31;

impl WebRadar {
    pub fn new() -> Self {
        WebRadar {
            players_info: WebPlayersInfo::new(Default::default()),
            timestamp: Instant::now(),
        }
    }

    fn generate_player_info(
        &self,
        ctx: &crate::UpdateContext,
        player_pawn: &Ptr<C_CSPlayerPawn>,
    ) -> anyhow::Result<Option<WebPlayerInfo>> {
        let player_pawn = player_pawn
            .read_schema()
            .with_context(|| obfstr!("failed to read player pawn data").to_string())?;

        let player_health = player_pawn.m_iHealth()?;
        if player_health <= 0 {
            return Ok(None); // TODO: Add death sign
        }

        /* Will be an instance of CSkeletonInstance */
        let game_screen_node = player_pawn
            .m_pGameSceneNode()?
            .cast::<CSkeletonInstance>()
            .read_schema()?;
        if game_screen_node.m_bDormant()? {
            return Ok(None);
        }

        let controller_handle = player_pawn.m_hController()?;
        let current_controller = ctx.cs2_entities.get_by_handle(&controller_handle)?;

        let player_team = player_pawn.m_iTeamNum()?;
        let player_name = if let Some(identity) = &current_controller {
            let player_controller = identity.entity()?.reference_schema()?;
            CStr::from_bytes_until_nul(&player_controller.m_iszPlayerName()?)
                .context("player name missing nul terminator")?
                .to_str()
                .context("invalid player name")?
                .to_string()
        } else {
            /*
             * This is the case for pawns which are not controlled by a player controller.
             * An example would be the main screen player pawns.
             *
             * Note: We're assuming, that uncontrolled player pawns are neglectable while being in a match as the do not occur.
             * Bots (and controller bots) always have a player pawn controller.
             */
            // log::warn!(
            //     "Handle at address {:p} has no valid controller!",
            //     &controller_handle
            // );
            return Ok(None);
        };

        let player_has_defuser = player_pawn
            .m_pItemServices()?
            .cast::<CCSPlayer_ItemServices>()
            .reference_schema()?
            .m_bHasDefuser()?;

        let position =
            nalgebra::Vector3::<f32>::from_column_slice(&game_screen_node.m_vecAbsOrigin()?);

        let weapon = player_pawn.m_pClippingWeapon()?.try_read_schema()?;
        let weapon_type = if let Some(weapon) = weapon {
            weapon
                .m_AttributeManager()?
                .m_Item()?
                .m_iItemDefinitionIndex()?
        } else {
            WeaponId::Knife.id()
        };

        let player_flashtime = player_pawn.m_flFlashBangTime()?;

        Ok(Some(WebPlayerInfo {
            controller_entity_id: controller_handle.get_entity_index(),
            team_id: player_team,

            player_name,
            player_has_defuser,
            player_health,
            weapon: WeaponId::from_id(weapon_type).unwrap_or(WeaponId::Unknown),
            player_flashtime,

            position: [position.x, position.y, position.z],
        }))
    }
}

impl Enhancement for WebRadar {
    fn update(&mut self, ctx: &crate::UpdateContext) -> anyhow::Result<()> {
        if self.timestamp.elapsed().as_millis() < UPDATE_TIME {
            return Ok(());
        }
        self.timestamp = Instant::now();
        self.players_info.players.clear();
        self.players_info.players.reserve(16);

        let local_player_controller = ctx
            .cs2_entities
            .get_local_player_controller()?
            .try_reference_schema()
            .with_context(|| obfstr!("failed to read local player controller").to_string())?;

        let local_player_controller = match local_player_controller {
            Some(controller) => controller,
            None => {
                /* We're currently not connected */
                return Ok(());
            }
        };

        if !local_player_controller.m_bPawnIsAlive()? {
            if ctx
                .cs2_entities
                .get_by_handle(&local_player_controller.m_hObserverPawn()?)?
                .is_none()
            {
                /* this is odd... */
                return Ok(());
            }
        }

        for entity_identity in ctx.cs2_entities.all_identities() {
            let entity_class = ctx
                .class_name_cache
                .lookup(&entity_identity.entity_class_info()?)?;
            if !entity_class
                .map(|name| *name == "C_CSPlayerPawn")
                .unwrap_or(false)
            {
                /* entity is not a player pawn */
                continue;
            }

            let player_pawn = entity_identity.entity_ptr::<C_CSPlayerPawn>()?;
            match self.generate_player_info(ctx, &player_pawn) {
                Ok(Some(info)) => self.players_info.players.push(info),
                Ok(None) => {}
                Err(error) => {
                    log::warn!(
                        "Failed to generate player pawn WebRadar info for {:X}: {:#}",
                        player_pawn.address()?,
                        error
                    );
                }
            }
        }

        let data = serde_json::to_string(&self.players_info).unwrap();
        for client in CLIENTS.lock().unwrap().iter() {
            client.do_send(MessageData { data: data.clone() });
        }

        Ok(())
    }

    fn render(&self, _settings: &AppSettings, _ui: &imgui::Ui, _view: &ViewController) {}
}
